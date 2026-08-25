//! Reverse inference — the centerpiece.
//!
//! GIE goes knowledge -> answer. This goes the other way:
//! canonical question/policy -> required predicates -> concrete evidence types
//! (privacy-ranked) -> only what is still MISSING given current subject state.

use crate::config::{PredicateDef, Registry};
use crate::domain::{AssuranceLevel, EvidenceState, Kind};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct EvidenceOptionOut {
    #[serde(rename = "type")]
    pub evidence_type: String,
    pub assurance: AssuranceLevel,
}

#[derive(Debug, Clone, Serialize)]
pub struct PredicatePlan {
    pub predicate: String,
    /// Privacy-ranked evidence options (preferred first).
    #[serde(rename = "evidenceOptions")]
    pub evidence_options: Vec<EvidenceOptionOut>,
    /// The single privacy-minimal type we actually request.
    pub preferred: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvidenceRequestPlan {
    pub canonical: String,
    pub kind: Kind,
    /// Flat privacy-minimal list: preferred type per still-missing predicate.
    #[serde(rename = "requiredEvidence")]
    pub required_evidence: Vec<String>,
    /// Per-predicate detail (config order).
    pub predicates: Vec<PredicatePlan>,
}

/// Evidence options ordered privacy-first: `prefer` type first, then config
/// order for the rest. Deterministic.
pub fn privacy_ordered(def: &PredicateDef) -> Vec<EvidenceOptionOut> {
    let mut out: Vec<EvidenceOptionOut> = Vec::new();
    if let Some(pref) = &def.prefer {
        if let Some(o) = def.evidence.iter().find(|o| &o.evidence_type == pref) {
            out.push(EvidenceOptionOut {
                evidence_type: o.evidence_type.clone(),
                assurance: o.assurance,
            });
        }
    }
    for o in &def.evidence {
        if out.iter().any(|x| x.evidence_type == o.evidence_type) {
            continue;
        }
        out.push(EvidenceOptionOut {
            evidence_type: o.evidence_type.clone(),
            assurance: o.assurance,
        });
    }
    out
}

/// The predicates a canonical id requires, in config order.
pub fn required_predicates(registry: &Registry, canonical: &str, kind: Kind) -> Vec<String> {
    match kind {
        Kind::Predicate => vec![canonical.to_string()],
        Kind::Policy => registry
            .policies
            .get(canonical)
            .map(|p| p.requires.clone())
            .unwrap_or_default(),
    }
}

/// Build an evidence request plan listing only predicates NOT yet established by
/// the supplied evidence states. `established` names predicates already TRUE.
pub fn build_plan(
    registry: &Registry,
    canonical: &str,
    kind: Kind,
    established: &BTreeMap<String, bool>,
) -> EvidenceRequestPlan {
    let mut predicates = Vec::new();
    let mut required_evidence = Vec::new();
    for pred in required_predicates(registry, canonical, kind) {
        if established.get(&pred).copied().unwrap_or(false) {
            continue; // already satisfied -> request nothing (data minimization)
        }
        let Some(def) = registry.predicates.get(&pred) else {
            continue;
        };
        let options = privacy_ordered(def);
        let preferred = options
            .first()
            .map(|o| o.evidence_type.clone())
            .unwrap_or_default();
        if !preferred.is_empty() {
            required_evidence.push(preferred.clone());
        }
        predicates.push(PredicatePlan {
            predicate: pred,
            evidence_options: options,
            preferred,
        });
    }
    EvidenceRequestPlan {
        canonical: canonical.to_string(),
        kind,
        required_evidence,
        predicates,
    }
}

/// Given a subject's known evidence (type->state), which predicates are already
/// established (an AVAILABLE piece of any of the predicate's evidence options)?
pub fn established_from_evidence(
    registry: &Registry,
    canonical: &str,
    kind: Kind,
    ev: &BTreeMap<String, EvidenceState>,
) -> BTreeMap<String, bool> {
    let mut out = BTreeMap::new();
    for pred in required_predicates(registry, canonical, kind) {
        let established = registry
            .predicates
            .get(&pred)
            .map(|def| {
                def.evidence
                    .iter()
                    .any(|o| ev.get(&o.evidence_type) == Some(&EvidenceState::Available))
            })
            .unwrap_or(false);
        out.insert(pred, established);
    }
    out
}
