//! Question resolution — Tier 1 deterministic registry + validation of Tier 2
//! LLM proposals. The LLM never runs here; it only *proposes* a canonical id
//! (in the Java layer). The engine *disposes*: it re-validates every proposal
//! against the registry. Guessing is impossible by construction.

use crate::config::Registry;
use crate::domain::Kind;
use serde::{Deserialize, Serialize};

/// How a question was resolved to its canonical id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResolvedBy {
    Registry,
    LlmValidated,
}

#[derive(Debug, Clone, Serialize)]
pub struct Resolution {
    pub canonical: String,
    pub kind: Kind,
    #[serde(rename = "resolvedBy")]
    pub resolved_by: ResolvedBy,
}

/// Normalize per the documented algorithm:
/// lowercase -> trim -> collapse internal whitespace -> strip terminal ?.!
pub fn normalize(input: &str) -> String {
    let lowered = input.to_lowercase();
    let collapsed = lowered.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed
        .trim_end_matches(['?', '.', '!'])
        .trim()
        .to_string()
}

/// Tier 1: exact alias match, or direct canonical id acceptance.
/// Returns None on miss (caller falls through to Tier 2).
pub fn resolve_tier1(registry: &Registry, question: &str) -> Option<Resolution> {
    let norm = normalize(question);

    // Step 2: exact match against normalized alias table.
    for q in &registry.questions {
        for alias in &q.aliases {
            if normalize(alias) == norm {
                return Some(Resolution {
                    canonical: q.canonical.clone(),
                    kind: q.kind,
                    resolved_by: ResolvedBy::Registry,
                });
            }
        }
        // Step 3: input already IS a canonical id (case-sensitive canonical match).
        if q.canonical == question.trim() {
            return Some(Resolution {
                canonical: q.canonical.clone(),
                kind: q.kind,
                resolved_by: ResolvedBy::Registry,
            });
        }
    }
    None
}

/// Validate a Tier-2 LLM proposal (G2). Accepted only if it exactly matches a
/// known canonical id. Confidence gating (HIGH) is enforced in the Java layer
/// *before* the proposal reaches the engine; the engine validates membership.
pub fn validate_proposal(registry: &Registry, proposed: &str) -> Option<Resolution> {
    registry.kind_of(proposed).map(|kind| Resolution {
        canonical: proposed.to_string(),
        kind,
        resolved_by: ResolvedBy::LlmValidated,
    })
}
