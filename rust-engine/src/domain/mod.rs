//! Core domain types. Deliberately explicit: states are enums, never bools.
//!
//! This engine is the *reverse* of a Graph-based Inference Engine (GIE):
//! GIE goes knowledge -> inference -> answer; here we go
//! question -> required predicates -> required evidence -> verification -> decision.

use serde::{Deserialize, Serialize};

/// Whether a canonical id names a single predicate or a multi-predicate policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Predicate,
    Policy,
}

/// eIDAS-style assurance levels. LOW kept for completeness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AssuranceLevel {
    High,
    Substantial,
    Low,
}

/// Explicit evidence state. Never collapsed to a boolean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EvidenceState {
    Available,
    Missing,
    Expired,
    Invalid,
    Revoked,
    Unknown,
}

/// Truth value of a predicate after evaluation. UNKNOWN is first-class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PredicateState {
    True,
    False,
    Unknown,
}

/// Final decision. REPLAY_DETECTED surfaces the single-use nonce violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Decision {
    Allow,
    Deny,
    Unknown,
    ReplayDetected,
}

/// A single piece of evidence: its type and current state. No raw attribute values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    #[serde(rename = "type")]
    pub evidence_type: String,
    pub state: EvidenceState,
}
