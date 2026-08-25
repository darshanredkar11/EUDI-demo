//! Axum HTTP API — the Spring-independent engine contract.
//!
//! Endpoints:
//!   POST /engine/resolve            question (+ optional proposedCanonical) -> resolution | 422
//!   POST /engine/validate-canonical proposedCanonical -> resolution | 422
//!   POST /engine/plan               canonical + subject + known evidence -> request plan + challenge
//!   POST /engine/evaluate           canonical + evidence (+ eval time) -> decision
//!   POST /engine/presentations      requestId + presentation -> verify + evaluate (replay enforced)
//!   GET  /engine/audits/{id}        audit passthrough
//!   GET  /health                    liveness

use crate::domain::{Decision, Evidence, Kind};
use crate::evidence::Presentation;
use crate::inference::EvidenceRequestPlan;
use crate::replay::{self, Challenge, NonceResult};
use crate::resolution::{self, ResolvedBy};
use crate::{
    build_audit, build_replay_audit, build_verification_failure_audit, parse_eval_time, AppState,
};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/engine/resolve", post(resolve))
        .route("/engine/validate-canonical", post(validate_canonical))
        .route("/engine/plan", post(plan))
        .route("/engine/evaluate", post(evaluate))
        .route("/engine/presentations", post(presentations))
        .route("/engine/issuer/issue", post(issuer_issue))
        .route("/engine/issuer/jwks", get(issuer_jwks))
        .route("/engine/audits/:id", get(get_audit))
        .route("/api-docs/openapi.json", get(openapi_json))
        .merge(
            utoipa_swagger_ui::SwaggerUi::new("/swagger-ui")
                .config(utoipa_swagger_ui::Config::new(["/api-docs/openapi.json"])),
        )
        .with_state(state)
}

/// Serve the raw OpenAPI document (loaded by the bundled Swagger UI).
async fn openapi_json() -> impl IntoResponse {
    Json(crate::openapi::openapi_json())
}

// ---- /engine/resolve ------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ResolveReq {
    question: String,
    #[serde(rename = "proposedCanonical", default)]
    proposed_canonical: Option<String>,
}

async fn resolve(State(st): State<AppState>, Json(req): Json<ResolveReq>) -> impl IntoResponse {
    // Tier 1: deterministic registry.
    if let Some(r) = resolution::resolve_tier1(&st.registry, &req.question) {
        return (StatusCode::OK, Json(json!(r))).into_response();
    }
    // Tier 2 validation: if a proposal was supplied, validate membership (G2).
    if let Some(p) = &req.proposed_canonical {
        if let Some(r) = resolution::validate_proposal(&st.registry, p) {
            return (StatusCode::OK, Json(json!(r))).into_response();
        }
    }
    unresolved(&st)
}

#[derive(Debug, Deserialize)]
struct ValidateReq {
    #[serde(rename = "proposedCanonical")]
    proposed_canonical: String,
}

async fn validate_canonical(
    State(st): State<AppState>,
    Json(req): Json<ValidateReq>,
) -> impl IntoResponse {
    match resolution::validate_proposal(&st.registry, &req.proposed_canonical) {
        Some(r) => (StatusCode::OK, Json(json!(r))).into_response(),
        None => unresolved(&st),
    }
}

fn unresolved(st: &AppState) -> axum::response::Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "error": "UNRESOLVED_QUESTION",
            "supportedQuestions": st.registry.supported_questions(),
        })),
    )
        .into_response()
}

// ---- /engine/plan ---------------------------------------------------------

#[derive(Debug, Deserialize)]
struct PlanReq {
    canonical: String,
    #[serde(rename = "subjectId")]
    subject_id: String,
    #[serde(rename = "knownEvidence", default)]
    known_evidence: Vec<Evidence>,
    #[serde(default)]
    audience: Option<String>,
    #[serde(rename = "resolvedBy", default)]
    resolved_by: Option<ResolvedBy>,
    #[serde(rename = "llmProposal", default)]
    llm_proposal: Option<Value>,
    #[serde(default)]
    question: Option<String>,
}

#[derive(Debug, Serialize)]
struct PlanResp {
    #[serde(rename = "requestId")]
    request_id: String,
    canonical: String,
    kind: Kind,
    #[serde(rename = "requiredEvidence")]
    required_evidence: Vec<String>,
    nonce: String,
    #[serde(rename = "expiresAt")]
    expires_at: String,
    challenge: Challenge,
    plan: EvidenceRequestPlan,
}

async fn plan(State(st): State<AppState>, Json(req): Json<PlanReq>) -> impl IntoResponse {
    let Some(kind) = st.registry.kind_of(&req.canonical) else {
        return unresolved(&st);
    };
    let mut ev = std::collections::BTreeMap::new();
    for e in &req.known_evidence {
        ev.insert(e.evidence_type.clone(), e.state);
    }
    let established =
        crate::inference::established_from_evidence(&st.registry, &req.canonical, kind, &ev);
    let plan = crate::inference::build_plan(&st.registry, &req.canonical, kind, &established);

    let now = OffsetDateTime::now_utc();
    let expires = now + Duration::seconds(replay::NONCE_TTL_SECONDS);
    let challenge = Challenge {
        request_id: Uuid::new_v4().to_string(),
        nonce: replay::generate_nonce(),
        audience: req.audience.unwrap_or_else(|| "relying-party-demo".to_string()),
        issued_at: now,
        expires_at: expires,
        canonical: req.canonical.clone(),
        kind,
        subject_id: req.subject_id.clone(),
        required_evidence: plan.required_evidence.clone(),
    };
    st.replay.put(challenge.clone());
    // resolvedBy / llmProposal / question are accepted for symmetry but the
    // plan itself is provenance-neutral; they thread through at evaluate time.
    let _ = (&req.resolved_by, &req.llm_proposal, &req.question);

    let resp = PlanResp {
        request_id: challenge.request_id.clone(),
        canonical: req.canonical,
        kind,
        required_evidence: plan.required_evidence.clone(),
        nonce: challenge.nonce.clone(),
        expires_at: expires.format(&Rfc3339).unwrap_or_default(),
        challenge,
        plan,
    };
    (StatusCode::OK, Json(json!(resp))).into_response()
}

// ---- /engine/evaluate -----------------------------------------------------

#[derive(Debug, Deserialize)]
struct EvaluateReq {
    canonical: String,
    #[serde(default)]
    evidence: Vec<Evidence>,
    #[serde(rename = "evaluationTime", default)]
    evaluation_time: Option<String>,
    #[serde(default)]
    question: Option<String>,
    #[serde(rename = "resolvedBy", default)]
    resolved_by: Option<ResolvedBy>,
    #[serde(rename = "llmProposal", default)]
    llm_proposal: Option<Value>,
}

async fn evaluate(State(st): State<AppState>, Json(req): Json<EvaluateReq>) -> impl IntoResponse {
    let Some(kind) = st.registry.kind_of(&req.canonical) else {
        return unresolved(&st);
    };
    let eval_time = match parse_eval_time(req.evaluation_time.as_deref()) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "VALIDATION_ERROR", "message": e})),
            )
                .into_response()
        }
    };
    let report =
        crate::policy::evaluate(&st.registry, &req.canonical, kind, &req.evidence, eval_time);
    let audit_id = Uuid::new_v4().to_string();
    let audit = build_audit(
        &st.registry,
        audit_id.clone(),
        req.question.unwrap_or_default(),
        req.resolved_by,
        req.llm_proposal,
        &report,
    );
    st.audit.record(audit);
    let mut body = serde_json::to_value(&report).unwrap();
    body.as_object_mut()
        .unwrap()
        .insert("auditId".to_string(), json!(audit_id));
    (StatusCode::OK, Json(body)).into_response()
}

// ---- /engine/presentations ------------------------------------------------

#[derive(Debug, Deserialize)]
struct PresentReq {
    #[serde(rename = "requestId")]
    request_id: String,
    presentation: Presentation,
    #[serde(rename = "evaluationTime", default)]
    evaluation_time: Option<String>,
    #[serde(default)]
    question: Option<String>,
    #[serde(rename = "resolvedBy", default)]
    resolved_by: Option<ResolvedBy>,
    #[serde(rename = "llmProposal", default)]
    llm_proposal: Option<Value>,
}

async fn presentations(
    State(st): State<AppState>,
    Json(req): Json<PresentReq>,
) -> impl IntoResponse {
    let now = OffsetDateTime::now_utc();
    let eval_time = match parse_eval_time(req.evaluation_time.as_deref()) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "VALIDATION_ERROR", "message": e})),
            )
                .into_response()
        }
    };

    // Peek the challenge; do NOT consume the nonce until verification succeeds.
    let challenge = match st.replay.get(&req.request_id) {
        Some(c) => c,
        None => return nonce_error(StatusCode::NOT_FOUND, "UNKNOWN_REQUEST"),
    };

    // Real SD-JWT verification (or mock in tests). Clock is injected → deterministic.
    let vr = st.verifier.verify(&req.presentation, &challenge, eval_time);
    let checks_json = serde_json::to_value(&vr.checks).unwrap_or_else(|_| json!([]));

    if !vr.ok {
        // A verification check failed → DENY; the nonce is NOT consumed.
        let failed = vr
            .failed_check
            .clone()
            .unwrap_or_else(|| "VERIFICATION_FAILED".to_string());
        let audit_id = Uuid::new_v4().to_string();
        st.audit.record(build_verification_failure_audit(
            audit_id.clone(),
            challenge.canonical.clone(),
            failed.clone(),
            vr.checks.clone(),
            vr.disclosure_count,
            eval_time.format(&Rfc3339).unwrap_or_default(),
        ));
        return (
            StatusCode::OK,
            Json(json!({
                "decision": Decision::Deny,
                "canonical": challenge.canonical,
                "failedCheck": failed,
                "verificationChecks": checks_json,
                "disclosureCount": vr.disclosure_count,
                "auditId": audit_id,
            })),
        )
            .into_response();
    }

    // Verification passed → consume the nonce (single-use) via the verified KB values.
    let (result, _c) =
        st.replay
            .validate_and_consume(&req.request_id, &vr.nonce, &vr.audience, now);
    match result {
        NonceResult::Ok => {}
        NonceResult::NonceAlreadyConsumed => {
            let audit_id = Uuid::new_v4().to_string();
            st.audit.record(build_replay_audit(
                audit_id.clone(),
                challenge.canonical.clone(),
                "NONCE_ALREADY_CONSUMED".to_string(),
            ));
            return (
                StatusCode::OK,
                Json(json!({
                    "decision": Decision::ReplayDetected,
                    "reason": "NONCE_ALREADY_CONSUMED",
                    "canonical": challenge.canonical,
                    "auditId": audit_id,
                })),
            )
                .into_response();
        }
        NonceResult::ExpiredNonce => return nonce_error(StatusCode::GONE, "EXPIRED_NONCE"),
        NonceResult::UnknownNonce => return nonce_error(StatusCode::NOT_FOUND, "UNKNOWN_REQUEST"),
        NonceResult::WrongAudience => return nonce_error(StatusCode::CONFLICT, "WRONG_AUDIENCE"),
        NonceResult::WrongRequestId => return nonce_error(StatusCode::CONFLICT, "WRONG_REQUEST_ID"),
    }

    let report = crate::policy::evaluate(
        &st.registry,
        &challenge.canonical,
        challenge.kind,
        &vr.verified_evidence,
        eval_time,
    );
    let audit_id = Uuid::new_v4().to_string();
    let mut audit = build_audit(
        &st.registry,
        audit_id.clone(),
        req.question.unwrap_or_default(),
        req.resolved_by,
        req.llm_proposal,
        &report,
    );
    audit.verification_checks = vr.checks.clone();
    audit.disclosure_count = Some(vr.disclosure_count);
    st.audit.record(audit);

    let mut body = serde_json::to_value(&report).unwrap();
    let obj = body.as_object_mut().unwrap();
    obj.insert("auditId".to_string(), json!(audit_id));
    obj.insert("verificationChecks".to_string(), checks_json);
    obj.insert("disclosureCount".to_string(), json!(vr.disclosure_count));
    (StatusCode::OK, Json(body)).into_response()
}

fn nonce_error(code: StatusCode, err: &str) -> axum::response::Response {
    (code, Json(json!({"error": err}))).into_response()
}

// ---- /engine/audits/:id ---------------------------------------------------

async fn get_audit(State(st): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match st.audit.get(&id) {
        Some(a) => (StatusCode::OK, Json(json!(a))).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "UNKNOWN_REQUEST", "message": "audit not found"})),
        )
            .into_response(),
    }
}

// ---- /engine/issuer/* -----------------------------------------------------

#[derive(Debug, Deserialize)]
struct IssueReq {
    #[serde(rename = "subjectId")]
    subject_id: String,
    /// Holder public key (EC P-256 JWK) for key binding (cnf).
    #[serde(rename = "holderJwk")]
    holder_jwk: Value,
}

/// Issue a demo PID SD-JWT VC bound to the holder's key. Real ES256 signature.
async fn issuer_issue(State(st): State<AppState>, Json(req): Json<IssueReq>) -> impl IntoResponse {
    if crate::crypto::verifying_from_jwk(&req.holder_jwk).is_err() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "VALIDATION_ERROR", "message": "invalid holderJwk (EC P-256 required)"})),
        )
            .into_response();
    }
    let claims = crate::issuer::demo_claims(&req.subject_id);
    let now = OffsetDateTime::now_utc();
    let issued = st.issuer.issue(&claims, req.holder_jwk, now);
    (StatusCode::OK, Json(json!(issued))).into_response()
}

/// Publish the issuer public key set (trust-list stand-in for this demo).
async fn issuer_jwks(State(st): State<AppState>) -> impl IntoResponse {
    (StatusCode::OK, Json(st.issuer.jwks())).into_response()
}
