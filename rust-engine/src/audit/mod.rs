//! Audit / provenance. Every decision (including UNKNOWN, REPLAY_DETECTED,
//! UNRESOLVED_QUESTION) emits an audit object. Explains WHY without storing PII:
//! evidence TYPES and STATES only, never raw attribute values.

use crate::domain::Decision;
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Serialize)]
pub struct Audit {
    #[serde(rename = "auditId")]
    pub audit_id: String,
    pub question: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<String>,
    #[serde(rename = "policyVersion", skip_serializing_if = "Option::is_none")]
    pub policy_version: Option<u32>,
    #[serde(rename = "resolvedBy", skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<String>,
    /// Present for LLM_VALIDATED resolutions: the raw proposal JSON (G5).
    #[serde(rename = "llmProposal", skip_serializing_if = "Option::is_none")]
    pub llm_proposal: Option<Value>,
    pub decision: Decision,
    /// predicate -> TRUE|FALSE|UNKNOWN (states, no values).
    pub predicates: BTreeMap<String, String>,
    #[serde(rename = "evidenceUsed")]
    pub evidence_used: Vec<String>,
    #[serde(rename = "evidenceMissing")]
    pub evidence_missing: Vec<String>,
    pub reasons: Vec<String>,
    #[serde(rename = "verificationChecks", skip_serializing_if = "Vec::is_empty", default)]
    pub verification_checks: Vec<crate::evidence::CheckResult>,
    #[serde(rename = "disclosureCount", skip_serializing_if = "Option::is_none")]
    pub disclosure_count: Option<usize>,
    #[serde(rename = "evaluationTime")]
    pub evaluation_time: String,
}

#[derive(Default)]
pub struct AuditLog {
    inner: Mutex<HashMap<String, Audit>>,
}

impl AuditLog {
    pub fn record(&self, audit: Audit) {
        self.inner.lock().insert(audit.audit_id.clone(), audit);
    }

    pub fn get(&self, id: &str) -> Option<Audit> {
        self.inner.lock().get(id).cloned()
    }
}
