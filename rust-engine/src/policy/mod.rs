//! Deterministic policy evaluation -> ALLOW | DENY | UNKNOWN.
//!
//! - DENY only when evidence affirmatively establishes falsity, or is INVALID/REVOKED.
//! - UNKNOWN when required evidence is MISSING/EXPIRED/unresolved. UNKNOWN is a
//!   legitimate first-class decision, never an error.
//! - No randomness, injected clock, deterministic ordering -> reproducible.

use crate::config::Registry;
use crate::domain::{Decision, Evidence, EvidenceState, Kind, PredicateState};
use crate::inference::{self, EvidenceRequestPlan};
use serde::Serialize;
use std::collections::BTreeMap;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize)]
pub struct DecisionReport {
    pub decision: Decision,
    pub canonical: String,
    pub kind: Kind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predicate: Option<String>,
    #[serde(rename = "policyVersion", skip_serializing_if = "Option::is_none")]
    pub policy_version: Option<u32>,
    /// Derived claims only (booleans). Never raw attribute values.
    #[serde(rename = "verifiedClaims")]
    pub verified_claims: BTreeMap<String, bool>,
    #[serde(rename = "satisfiedPredicates")]
    pub satisfied_predicates: Vec<String>,
    #[serde(rename = "missingPredicates")]
    pub missing_predicates: Vec<String>,
    #[serde(rename = "evidenceUsed")]
    pub evidence_used: Vec<Evidence>,
    #[serde(rename = "evidenceIgnored")]
    pub evidence_ignored: Vec<Evidence>,
    pub reasons: Vec<String>,
    #[serde(rename = "evaluationTime")]
    pub evaluation_time: String,
    #[serde(rename = "evidenceRequestPlan", skip_serializing_if = "Option::is_none")]
    pub evidence_request_plan: Option<EvidenceRequestPlan>,
}

struct PredicateEval {
    state: PredicateState,
    used: Option<Evidence>,
    reason: String,
}

fn evaluate_predicate(
    registry: &Registry,
    pred: &str,
    ev: &BTreeMap<String, EvidenceState>,
) -> PredicateEval {
    let Some(def) = registry.predicates.get(pred) else {
        return PredicateEval {
            state: PredicateState::Unknown,
            used: None,
            reason: format!("{pred} is not a known predicate"),
        };
    };
    let options = inference::privacy_ordered(def);
    let mut affirmative_false: Option<(String, EvidenceState)> = None;

    for opt in &options {
        match ev.get(&opt.evidence_type) {
            Some(EvidenceState::Available) => {
                return PredicateEval {
                    state: PredicateState::True,
                    used: Some(Evidence {
                        evidence_type: opt.evidence_type.clone(),
                        state: EvidenceState::Available,
                    }),
                    reason: format!(
                        "{pred} established (evidence: {}, state: AVAILABLE)",
                        opt.evidence_type
                    ),
                };
            }
            Some(s @ (EvidenceState::Invalid | EvidenceState::Revoked)) => {
                affirmative_false.get_or_insert((opt.evidence_type.clone(), *s));
            }
            _ => {}
        }
    }

    if let Some((etype, state)) = affirmative_false {
        return PredicateEval {
            state: PredicateState::False,
            used: Some(Evidence {
                evidence_type: etype.clone(),
                state,
            }),
            reason: format!(
                "{pred} refuted (evidence: {etype}, state: {})",
                state_str(state)
            ),
        };
    }

    // Not established, not refuted -> UNKNOWN. Report the preferred option's state.
    let (etype, state) = options
        .first()
        .map(|o| {
            (
                o.evidence_type.clone(),
                ev.get(&o.evidence_type)
                    .copied()
                    .unwrap_or(EvidenceState::Missing),
            )
        })
        .unwrap_or_else(|| ("<none>".to_string(), EvidenceState::Missing));
    PredicateEval {
        state: PredicateState::Unknown,
        used: None,
        reason: format!(
            "{pred} cannot currently be established (evidence: {etype}, state: {})",
            state_str(state)
        ),
    }
}

fn state_str(s: EvidenceState) -> &'static str {
    match s {
        EvidenceState::Available => "AVAILABLE",
        EvidenceState::Missing => "MISSING",
        EvidenceState::Expired => "EXPIRED",
        EvidenceState::Invalid => "INVALID",
        EvidenceState::Revoked => "REVOKED",
        EvidenceState::Unknown => "UNKNOWN",
    }
}

/// Evaluate a resolved canonical (predicate or policy) against supplied evidence.
pub fn evaluate(
    registry: &Registry,
    canonical: &str,
    kind: Kind,
    evidence: &[Evidence],
    evaluation_time: OffsetDateTime,
) -> DecisionReport {
    // Build lookup; deterministic (last state for a type wins, config-independent).
    let mut ev: BTreeMap<String, EvidenceState> = BTreeMap::new();
    for e in evidence {
        ev.insert(e.evidence_type.clone(), e.state);
    }

    let preds = inference::required_predicates(registry, canonical, kind);
    let mut satisfied = Vec::new();
    let mut missing = Vec::new();
    let mut any_false = false;
    let mut reasons = Vec::new();
    let mut verified_claims = BTreeMap::new();
    let mut used: Vec<Evidence> = Vec::new();
    let mut used_types: Vec<String> = Vec::new();
    let mut established: BTreeMap<String, bool> = BTreeMap::new();

    for pred in &preds {
        let eval = evaluate_predicate(registry, pred, &ev);
        reasons.push(eval.reason);
        match eval.state {
            PredicateState::True => {
                satisfied.push(pred.clone());
                verified_claims.insert(pred.clone(), true);
                established.insert(pred.clone(), true);
                if let Some(u) = eval.used {
                    used_types.push(u.evidence_type.clone());
                    used.push(u);
                }
            }
            PredicateState::False => {
                any_false = true;
                established.insert(pred.clone(), false);
                if let Some(u) = eval.used {
                    used_types.push(u.evidence_type.clone());
                    used.push(u);
                }
            }
            PredicateState::Unknown => {
                missing.push(pred.clone());
                established.insert(pred.clone(), false);
            }
        }
    }

    let decision = if any_false {
        Decision::Deny
    } else if missing.is_empty() {
        Decision::Allow
    } else {
        Decision::Unknown
    };

    // Evidence provided but not used by any predicate.
    let evidence_ignored: Vec<Evidence> = evidence
        .iter()
        .filter(|e| !used_types.contains(&e.evidence_type))
        .cloned()
        .collect();

    let policy_version = match kind {
        Kind::Policy => registry.policies.get(canonical).map(|p| p.version),
        Kind::Predicate => None,
    };
    let predicate = match kind {
        Kind::Predicate => Some(canonical.to_string()),
        Kind::Policy => None,
    };

    let evidence_request_plan = if decision == Decision::Unknown {
        Some(inference::build_plan(registry, canonical, kind, &established))
    } else {
        None
    };

    DecisionReport {
        decision,
        canonical: canonical.to_string(),
        kind,
        predicate,
        policy_version,
        verified_claims,
        satisfied_predicates: satisfied,
        missing_predicates: missing,
        evidence_used: used,
        evidence_ignored,
        reasons,
        evaluation_time: evaluation_time
            .format(&Rfc3339)
            .unwrap_or_else(|_| "".to_string()),
        evidence_request_plan,
    }
}
