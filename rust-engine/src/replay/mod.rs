//! Replay protection — REAL, not mocked.
//!
//! Each evidence request creates a Challenge with a cryptographically secure,
//! single-use nonce and a short TTL. A presentation must echo the exact
//! request_id + nonce + audience. The nonce is consumed atomically on first
//! successful verification.

use crate::domain::Kind;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use parking_lot::Mutex;
use time::OffsetDateTime;

/// Default nonce time-to-live: 5 minutes.
pub const NONCE_TTL_SECONDS: i64 = 300;

/// A challenge bound to a specific resolved question/policy and subject.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Challenge {
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub nonce: String,
    pub audience: String,
    #[serde(rename = "issuedAt", with = "time::serde::rfc3339")]
    pub issued_at: OffsetDateTime,
    #[serde(rename = "expiresAt", with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    // Bound context so /engine/presentations can evaluate without re-resolving.
    pub canonical: String,
    pub kind: Kind,
    #[serde(rename = "subjectId")]
    pub subject_id: String,
    #[serde(rename = "requiredEvidence")]
    pub required_evidence: Vec<String>,
}

/// Outcome of validating (and consuming) a nonce. Distinct machine-readable codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NonceResult {
    Ok,
    UnknownNonce,
    ExpiredNonce,
    NonceAlreadyConsumed,
    WrongAudience,
    WrongRequestId,
}

/// Replay store seam. Redis-swappable later; in-memory now.
pub trait ReplayStore: Send + Sync {
    fn put(&self, challenge: Challenge);
    fn get(&self, request_id: &str) -> Option<Challenge>;
    /// Validate + atomically consume on OK.
    fn validate_and_consume(
        &self,
        request_id: &str,
        nonce: &str,
        audience: &str,
        now: OffsetDateTime,
    ) -> (NonceResult, Option<Challenge>);
}

struct Entry {
    challenge: Challenge,
    consumed: bool,
}

pub struct InMemoryReplayStore {
    inner: Mutex<HashMap<String, Entry>>,
}

impl Default for InMemoryReplayStore {
    fn default() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }
}

impl ReplayStore for InMemoryReplayStore {
    fn put(&self, challenge: Challenge) {
        let mut g = self.inner.lock();
        g.insert(
            challenge.request_id.clone(),
            Entry {
                challenge,
                consumed: false,
            },
        );
    }

    fn get(&self, request_id: &str) -> Option<Challenge> {
        let g = self.inner.lock();
        g.get(request_id).map(|e| e.challenge.clone())
    }

    fn validate_and_consume(
        &self,
        request_id: &str,
        nonce: &str,
        audience: &str,
        now: OffsetDateTime,
    ) -> (NonceResult, Option<Challenge>) {
        let mut g = self.inner.lock();
        let entry = match g.get_mut(request_id) {
            Some(e) => e,
            None => return (NonceResult::UnknownNonce, None),
        };
        if entry.challenge.nonce != nonce {
            // request_id known but nonce mismatch -> treat as unknown nonce
            return (NonceResult::UnknownNonce, None);
        }
        if entry.challenge.request_id != request_id {
            return (NonceResult::WrongRequestId, None);
        }
        if entry.challenge.audience != audience {
            return (NonceResult::WrongAudience, None);
        }
        if now > entry.challenge.expires_at {
            return (NonceResult::ExpiredNonce, Some(entry.challenge.clone()));
        }
        if entry.consumed {
            return (
                NonceResult::NonceAlreadyConsumed,
                Some(entry.challenge.clone()),
            );
        }
        // Atomic consume: still holding the lock.
        entry.consumed = true;
        (NonceResult::Ok, Some(entry.challenge.clone()))
    }
}

/// Generate a cryptographically secure nonce (256-bit, hex).
pub fn generate_nonce() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
