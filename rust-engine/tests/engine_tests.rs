//! Engine tests: resolution, reverse inference, evaluation, replay, determinism,
//! plus one integration test hitting the live HTTP API.

use engine::api;
use engine::config::Registry;
use engine::domain::{Decision, Evidence, EvidenceState, Kind};
use engine::resolution::{self, ResolvedBy};
use engine::{parse_eval_time, policy, AppState};
use std::path::PathBuf;

fn registry() -> Registry {
    Registry::load(&PathBuf::from("config")).expect("load config")
}

fn ev(t: &str, s: EvidenceState) -> Evidence {
    Evidence {
        evidence_type: t.to_string(),
        state: s,
    }
}

#[test]
fn tier1_alias_and_canonical_and_miss() {
    let r = registry();
    // alias, with punctuation + case + whitespace normalization
    let res = resolution::resolve_tier1(&r, "  Is This User Over 18?  ").unwrap();
    assert_eq!(res.canonical, "age_over_18");
    assert_eq!(res.kind, Kind::Predicate);
    assert_eq!(res.resolved_by, ResolvedBy::Registry);

    // demo-friendly alias maps to same predicate
    assert_eq!(
        resolution::resolve_tier1(&r, "can this user buy alcohol")
            .unwrap()
            .canonical,
        "age_over_18"
    );

    // direct canonical id accepted
    assert_eq!(
        resolution::resolve_tier1(&r, "BANK_ACCOUNT_OPENING_V1")
            .unwrap()
            .canonical,
        "BANK_ACCOUNT_OPENING_V1"
    );

    // miss
    assert!(resolution::resolve_tier1(&r, "can this user pilot a plane").is_none());
}

#[test]
fn proposal_validation_membership_only() {
    let r = registry();
    let ok = resolution::validate_proposal(&r, "age_over_18").unwrap();
    assert_eq!(ok.resolved_by, ResolvedBy::LlmValidated);
    // unknown id rejected
    assert!(resolution::validate_proposal(&r, "pilot_a_plane").is_none());
}

#[test]
fn predicate_allow_unknown_deny() {
    let r = registry();
    let t = parse_eval_time(None).unwrap();

    let allow = policy::evaluate(
        &r,
        "age_over_18",
        Kind::Predicate,
        &[ev("AGE_ATTESTATION", EvidenceState::Available)],
        t,
    );
    assert_eq!(allow.decision, Decision::Allow);
    assert_eq!(allow.verified_claims.get("age_over_18"), Some(&true));
    // privacy: derived claim only, and no DOB anywhere
    let json = serde_json::to_string(&allow).unwrap();
    assert!(!json.to_lowercase().contains("date_of_birth"));
    assert!(!json.to_lowercase().contains("dob"));

    let unknown = policy::evaluate(&r, "age_over_18", Kind::Predicate, &[], t);
    assert_eq!(unknown.decision, Decision::Unknown);
    assert!(unknown.evidence_request_plan.is_some());

    let deny = policy::evaluate(
        &r,
        "age_over_18",
        Kind::Predicate,
        &[ev("AGE_ATTESTATION", EvidenceState::Revoked)],
        t,
    );
    assert_eq!(deny.decision, Decision::Deny);
}

#[test]
fn policy_unknown_then_allow_with_plan() {
    let r = registry();
    let t = parse_eval_time(None).unwrap();
    let partial = vec![
        ev("GOVERNMENT_IDENTITY", EvidenceState::Available),
        ev("AGE_ATTESTATION", EvidenceState::Available),
        ev("CONSENT_RECORD", EvidenceState::Available),
        // RESIDENCE_CREDENTIAL missing
    ];
    let u = policy::evaluate(&r, "BANK_ACCOUNT_OPENING_V1", Kind::Policy, &partial, t);
    assert_eq!(u.decision, Decision::Unknown);
    assert_eq!(u.missing_predicates, vec!["eu_resident".to_string()]);
    let plan = u.evidence_request_plan.unwrap();
    assert_eq!(plan.required_evidence, vec!["RESIDENCE_CREDENTIAL".to_string()]);
    assert_eq!(u.policy_version, Some(1));

    let mut full = partial.clone();
    full.push(ev("RESIDENCE_CREDENTIAL", EvidenceState::Available));
    let a = policy::evaluate(&r, "BANK_ACCOUNT_OPENING_V1", Kind::Policy, &full, t);
    assert_eq!(a.decision, Decision::Allow);
    assert!(a.missing_predicates.is_empty());
}

#[test]
fn age_prefers_privacy_minimal_evidence() {
    let r = registry();
    let t = parse_eval_time(None).unwrap();
    // Both AGE_ATTESTATION and GOVERNMENT_IDENTITY available -> uses the
    // privacy-minimal AGE_ATTESTATION, ignores GOVERNMENT_IDENTITY for this predicate.
    let d = policy::evaluate(
        &r,
        "age_over_18",
        Kind::Predicate,
        &[
            ev("AGE_ATTESTATION", EvidenceState::Available),
            ev("GOVERNMENT_IDENTITY", EvidenceState::Available),
        ],
        t,
    );
    assert_eq!(d.decision, Decision::Allow);
    assert_eq!(d.evidence_used.len(), 1);
    assert_eq!(d.evidence_used[0].evidence_type, "AGE_ATTESTATION");
}

#[test]
fn determinism_byte_identical() {
    let r = registry();
    // frozen clock
    let t = parse_eval_time(Some("2020-01-01T00:00:00Z")).unwrap();
    let evidence = vec![ev("AGE_ATTESTATION", EvidenceState::Available)];
    let a = policy::evaluate(&r, "age_over_18", Kind::Predicate, &evidence, t);
    let b = policy::evaluate(&r, "age_over_18", Kind::Predicate, &evidence, t);
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
}

// ---- HTTP integration -----------------------------------------------------

#[tokio::test]
async fn http_resolve_plan_present_replay() {
    // This legacy state-based flow exercises the router with the mock verifier;
    // the real SD-JWT path is covered in sdjwt_tests.rs.
    let state = AppState::with_verifier(
        registry(),
        std::sync::Arc::new(engine::evidence::MockCredentialVerifier),
    );
    let app = api::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{addr}");
    let c = reqwest::Client::new();

    // resolve
    let r: serde_json::Value = c
        .post(format!("{base}/engine/resolve"))
        .json(&serde_json::json!({"question": "is this user over 18"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r["canonical"], "age_over_18");
    assert_eq!(r["resolvedBy"], "REGISTRY");

    // plan -> requestId + nonce
    let p: serde_json::Value = c
        .post(format!("{base}/engine/plan"))
        .json(&serde_json::json!({"canonical":"age_over_18","subjectId":"user-123"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let request_id = p["requestId"].as_str().unwrap().to_string();
    let nonce = p["nonce"].as_str().unwrap().to_string();
    assert_eq!(p["requiredEvidence"][0], "AGE_ATTESTATION");

    let presentation = serde_json::json!({
        "requestId": request_id,
        "nonce": nonce,
        "audience": "relying-party-demo",
        "evidence": [{"type":"AGE_ATTESTATION","state":"AVAILABLE"}],
        "signatures": {"mock": true}
    });

    // first presentation -> ALLOW
    let d1: serde_json::Value = c
        .post(format!("{base}/engine/presentations"))
        .json(&serde_json::json!({"requestId": request_id, "presentation": presentation}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(d1["decision"], "ALLOW");
    assert_eq!(d1["verifiedClaims"]["age_over_18"], true);

    // replay -> REPLAY_DETECTED
    let d2: serde_json::Value = c
        .post(format!("{base}/engine/presentations"))
        .json(&serde_json::json!({"requestId": request_id, "presentation": presentation}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(d2["decision"], "REPLAY_DETECTED");
    assert_eq!(d2["reason"], "NONCE_ALREADY_CONSUMED");

    // audit passthrough
    let audit_id = d1["auditId"].as_str().unwrap();
    let a: serde_json::Value = c
        .get(format!("{base}/engine/audits/{audit_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(a["decision"], "ALLOW");
}
