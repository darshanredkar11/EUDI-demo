//! Identity Evidence Inference Engine — deterministic core.
//!
//! Architecture: this Rust service owns ALL business logic (question registry,
//! predicates, policies, reverse inference, evaluation, replay protection,
//! audit). The Java layer is a thin orchestrator. No AI dependency lives here:
//! the LLM is an untrusted proposer in the Java layer; this engine only
//! *validates* proposals against the registry (see `resolution`).

pub mod audit;
pub mod config;
pub mod crypto;
pub mod domain;
pub mod evidence;
pub mod inference;
pub mod issuer;
pub mod policy;
pub mod replay;
pub mod resolution;
pub mod openapi;

pub mod api;

use crate::audit::{Audit, AuditLog};
use crate::config::Registry;
use crate::domain::Decision;
use crate::evidence::{CheckResult, CredentialVerifier, SdJwtCredentialVerifier};
use crate::issuer::Issuer;
use crate::policy::DecisionReport;
use crate::replay::{InMemoryReplayStore, ReplayStore};
use crate::resolution::ResolvedBy;
use serde_json::Value;
use std::sync::Arc;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Shared application state. All seams (replay store, verifier) are trait
/// objects so real implementations swap in without touching handlers.
#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<Registry>,
    pub replay: Arc<dyn ReplayStore>,
    pub verifier: Arc<dyn CredentialVerifier>,
    pub audit: Arc<AuditLog>,
    pub issuer: Arc<Issuer>,
}

impl AppState {
    /// Production wiring: real SD-JWT verifier pinned to this issuer's key.
    pub fn new(registry: Registry) -> Self {
        let issuer = Arc::new(Issuer::new());
        let verifier: Arc<dyn CredentialVerifier> =
            Arc::new(SdJwtCredentialVerifier::new(issuer.verifying_key(), issuer.iss.clone()));
        Self {
            registry: Arc::new(registry),
            replay: Arc::new(InMemoryReplayStore::default()),
            verifier,
            audit: Arc::new(AuditLog::default()),
            issuer,
        }
    }

    /// Test wiring: inject any verifier (e.g. `MockCredentialVerifier`).
    pub fn with_verifier(registry: Registry, verifier: Arc<dyn CredentialVerifier>) -> Self {
        Self {
            registry: Arc::new(registry),
            replay: Arc::new(InMemoryReplayStore::default()),
            verifier,
            audit: Arc::new(AuditLog::default()),
            issuer: Arc::new(Issuer::new()),
        }
    }
}

/// Parse an optional RFC3339 evaluation time; default to now (UTC).
pub fn parse_eval_time(s: Option<&str>) -> Result<OffsetDateTime, String> {
    match s {
        Some(v) => OffsetDateTime::parse(v, &Rfc3339).map_err(|e| format!("bad evaluationTime: {e}")),
        None => Ok(OffsetDateTime::now_utc()),
    }
}

/// Build an audit object from a decision report + resolution provenance.
pub fn build_audit(
    registry: &Registry,
    audit_id: String,
    question: String,
    resolved_by: Option<ResolvedBy>,
    llm_proposal: Option<Value>,
    report: &DecisionReport,
) -> Audit {
    let preds = inference::required_predicates(registry, &report.canonical, report.kind);
    let mut predicates = std::collections::BTreeMap::new();
    for p in &preds {
        let state = if report.satisfied_predicates.contains(p) {
            "TRUE"
        } else if report.missing_predicates.contains(p) {
            "UNKNOWN"
        } else {
            "FALSE"
        };
        predicates.insert(p.clone(), state.to_string());
    }
    let evidence_used: Vec<String> = report
        .evidence_used
        .iter()
        .map(|e| format!("{}:{}", e.evidence_type, state_label(e.state)))
        .collect();
    let evidence_missing: Vec<String> = report
        .evidence_request_plan
        .as_ref()
        .map(|p| p.required_evidence.clone())
        .unwrap_or_default();

    let (policy, policy_version) = match report.kind {
        domain::Kind::Policy => (Some(report.canonical.clone()), report.policy_version),
        domain::Kind::Predicate => (None, None),
    };

    Audit {
        audit_id,
        question,
        canonical: Some(report.canonical.clone()),
        policy,
        policy_version,
        resolved_by: resolved_by.map(|r| serde_json::to_value(r).unwrap().as_str().unwrap().to_string()),
        llm_proposal,
        decision: report.decision,
        predicates,
        evidence_used,
        evidence_missing,
        reasons: report.reasons.clone(),
        verification_checks: Vec::new(),
        disclosure_count: None,
        evaluation_time: report.evaluation_time.clone(),
    }
}

fn state_label(s: domain::EvidenceState) -> &'static str {
    use domain::EvidenceState::*;
    match s {
        Available => "AVAILABLE",
        Missing => "MISSING",
        Expired => "EXPIRED",
        Invalid => "INVALID",
        Revoked => "REVOKED",
        Unknown => "UNKNOWN",
    }
}

/// Audit for a replay-detected presentation (no evidence evaluated).
pub fn build_replay_audit(audit_id: String, canonical: String, reason: String) -> Audit {
    Audit {
        audit_id,
        question: String::new(),
        canonical: Some(canonical),
        policy: None,
        policy_version: None,
        resolved_by: None,
        llm_proposal: None,
        decision: Decision::ReplayDetected,
        predicates: std::collections::BTreeMap::new(),
        evidence_used: Vec::new(),
        evidence_missing: Vec::new(),
        reasons: vec![reason],
        verification_checks: Vec::new(),
        disclosure_count: None,
        evaluation_time: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_default(),
    }
}

/// Audit for a failed SD-JWT verification (crypto check failed -> DENY).
#[allow(clippy::too_many_arguments)]
pub fn build_verification_failure_audit(
    audit_id: String,
    canonical: String,
    failed_check: String,
    checks: Vec<CheckResult>,
    disclosure_count: usize,
    evaluation_time: String,
) -> Audit {
    Audit {
        audit_id,
        question: String::new(),
        canonical: Some(canonical),
        policy: None,
        policy_version: None,
        resolved_by: None,
        llm_proposal: None,
        decision: Decision::Deny,
        predicates: std::collections::BTreeMap::new(),
        evidence_used: Vec::new(),
        evidence_missing: Vec::new(),
        reasons: vec![format!("verification failed: {failed_check}")],
        verification_checks: checks,
        disclosure_count: Some(disclosure_count),
        evaluation_time,
    }
}
