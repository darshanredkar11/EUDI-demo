//! Privacy-preserving verification.
//!
//! RAW_ATTRIBUTE (e.g. date_of_birth) vs DERIVED_CLAIM (e.g. age_over_18=true):
//! the verifier consumes a raw disclosure but emits only the derived boolean via
//! evidence state. Raw attribute VALUES never leave this module.
//!
//! Two verifiers behind the `CredentialVerifier` seam:
//!  - `SdJwtCredentialVerifier` — REAL ES256 SD-JWT VC verification (live wiring).
//!  - `MockCredentialVerifier` — trusts presented states (kept for tests).

use crate::crypto;
use crate::domain::{Evidence, EvidenceState};
use crate::replay::Challenge;
use p256::ecdsa::VerifyingKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use time::OffsetDateTime;

/// The kind of information an attribute conveys — used to enforce minimization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttributeKind {
    /// e.g. date_of_birth — must never appear in a business response.
    RawAttribute,
    /// e.g. age_over_18 = true — the minimal disclosure we prefer.
    DerivedClaim,
}

/// A wallet presentation. `sdJwtVp` carries a real SD-JWT VC presentation
/// (issuer JWT + selected disclosures + KB-JWT). The legacy `evidence`/`nonce`
/// fields feed the mock verifier only.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Presentation {
    #[serde(rename = "requestId", default)]
    pub request_id: String,
    #[serde(default)]
    pub nonce: String,
    #[serde(default)]
    pub audience: String,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub signatures: Value,
    /// Real SD-JWT VC presentation: `<issuer-jwt>~<disclosure>*~<kb-jwt>`.
    #[serde(rename = "sdJwtVp", default)]
    pub sd_jwt_vp: Option<String>,
}

/// One named verification check, recorded in the audit and response.
#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub name: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub detail: String,
}

impl CheckResult {
    fn pass(name: &str, detail: &str) -> Self {
        Self { name: name.into(), ok: true, detail: detail.into() }
    }
    fn fail(name: &str, detail: &str) -> Self {
        Self { name: name.into(), ok: false, detail: detail.into() }
    }
}

#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// True iff every cryptographic check (a–e) passed.
    pub ok: bool,
    /// Named failing check when `ok == false` (e.g. INVALID_SIGNATURE).
    pub failed_check: Option<String>,
    pub checks: Vec<CheckResult>,
    /// Evidence type+state derived from disclosures — NO raw values.
    pub verified_evidence: Vec<Evidence>,
    /// Nonce + audience extracted from the (verified) KB-JWT, for replay consume.
    pub nonce: String,
    pub audience: String,
    pub disclosure_count: usize,
}

/// Verification seam. `now` is the injected evaluation clock (determinism).
pub trait CredentialVerifier: Send + Sync {
    fn verify(
        &self,
        presentation: &Presentation,
        challenge: &Challenge,
        now: OffsetDateTime,
    ) -> VerificationResult;
}

// ---------------------------------------------------------------------------
// MOCK verifier (tests only) — no cryptography, trusts presented states.
// ---------------------------------------------------------------------------

pub struct MockCredentialVerifier;

impl CredentialVerifier for MockCredentialVerifier {
    fn verify(
        &self,
        presentation: &Presentation,
        _challenge: &Challenge,
        _now: OffsetDateTime,
    ) -> VerificationResult {
        let verified: Vec<Evidence> = presentation
            .evidence
            .iter()
            .filter(|e| e.state == EvidenceState::Available)
            .cloned()
            .collect();
        VerificationResult {
            ok: true,
            failed_check: None,
            checks: vec![CheckResult::pass("MOCK_VERIFIER", "states trusted (test seam)")],
            verified_evidence: verified,
            nonce: presentation.nonce.clone(),
            audience: presentation.audience.clone(),
            disclosure_count: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// REAL SD-JWT VC verifier.
// ---------------------------------------------------------------------------

/// Verifies IETF SD-JWT VC presentations with ES256. The issuer public key is
/// pinned at construction (stands in for a trust list in this demo).
pub struct SdJwtCredentialVerifier {
    issuer_vk: VerifyingKey,
    issuer_iss: String,
}

impl SdJwtCredentialVerifier {
    pub fn new(issuer_vk: VerifyingKey, issuer_iss: String) -> Self {
        Self { issuer_vk, issuer_iss }
    }
}

fn fail(code: &str, detail: &str, mut checks: Vec<CheckResult>, dcount: usize) -> VerificationResult {
    checks.push(CheckResult::fail(code, detail));
    VerificationResult {
        ok: false,
        failed_check: Some(code.to_string()),
        checks,
        verified_evidence: Vec::new(),
        nonce: String::new(),
        audience: String::new(),
        disclosure_count: dcount,
    }
}

impl CredentialVerifier for SdJwtCredentialVerifier {
    fn verify(
        &self,
        presentation: &Presentation,
        challenge: &Challenge,
        now: OffsetDateTime,
    ) -> VerificationResult {
        let mut checks: Vec<CheckResult> = Vec::new();

        let vp = match &presentation.sd_jwt_vp {
            Some(v) if !v.is_empty() => v,
            _ => return fail("MISSING_SD_JWT_VP", "no SD-JWT VP present", checks, 0),
        };

        // Split: <issuer-jwt> ~ <disclosure>* ~ <kb-jwt>
        let parts: Vec<&str> = vp.split('~').collect();
        if parts.len() < 2 {
            return fail("MALFORMED_VP", "expected issuer JWT and KB-JWT", checks, 0);
        }
        let issuer_jwt = parts[0];
        let kb_jwt = parts[parts.len() - 1];
        let disclosure_parts: Vec<&str> = parts[1..parts.len() - 1].to_vec();
        if kb_jwt.is_empty() {
            return fail("MALFORMED_VP", "missing KB-JWT (presentation not key-bound)", checks, 0);
        }
        let dcount = disclosure_parts.iter().filter(|d| !d.is_empty()).count();

        // (a) Issuer signature valid against pinned issuer key (trust list stand-in).
        let issuer_payload = match crypto::jws_verify(&self.issuer_vk, issuer_jwt) {
            Ok(p) => p,
            Err(_) => return fail("INVALID_SIGNATURE", "issuer JWT signature invalid", checks, dcount),
        };
        checks.push(CheckResult::pass("ISSUER_SIGNATURE", "valid against issuer JWKS"));

        // Issuer identity (trust anchor).
        if issuer_payload.get("iss").and_then(|v| v.as_str()) != Some(self.issuer_iss.as_str()) {
            return fail("UNTRUSTED_ISSUER", "iss does not match trust anchor", checks, dcount);
        }

        // (b) Each disclosure hashes to a digest present in _sd.
        let sd_set: BTreeSet<String> = issuer_payload
            .get("_sd")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|d| d.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let mut disclosed: BTreeMap<String, Value> = BTreeMap::new();
        for d in &disclosure_parts {
            if d.is_empty() {
                continue;
            }
            let digest = crypto::sha256_b64url(d.as_bytes());
            if !sd_set.contains(&digest) {
                return fail("UNKNOWN_DISCLOSURE", "disclosure digest not in _sd", checks, dcount);
            }
            let raw = match crypto::b64url_decode(d) {
                Ok(b) => b,
                Err(_) => return fail("UNKNOWN_DISCLOSURE", "disclosure not base64url", checks, dcount),
            };
            let arr: Value = match serde_json::from_slice(&raw) {
                Ok(v) => v,
                Err(_) => return fail("UNKNOWN_DISCLOSURE", "disclosure not JSON", checks, dcount),
            };
            // [salt, name, value]
            let name = arr.get(1).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let value = arr.get(2).cloned().unwrap_or(Value::Null);
            if name.is_empty() {
                return fail("UNKNOWN_DISCLOSURE", "malformed disclosure array", checks, dcount);
            }
            disclosed.insert(name, value);
        }
        checks.push(CheckResult::pass(
            "DISCLOSURE_DIGESTS",
            &format!("{dcount} disclosure(s) matched _sd"),
        ));

        // (c) Credential validity against the injected clock.
        let exp = issuer_payload.get("exp").and_then(|v| v.as_i64()).unwrap_or(0);
        if now.unix_timestamp() > exp {
            return fail("EXPIRED_CREDENTIAL", "credential exp is in the past", checks, dcount);
        }
        checks.push(CheckResult::pass("CREDENTIAL_VALIDITY", "not expired at evaluationTime"));

        // (d) Key binding: KB-JWT signed by cnf key, aud + sd_hash match.
        let holder_jwk = match issuer_payload.pointer("/cnf/jwk") {
            Some(j) => j.clone(),
            None => return fail("KEY_BINDING_MISMATCH", "missing cnf.jwk", checks, dcount),
        };
        let holder_vk = match crypto::verifying_from_jwk(&holder_jwk) {
            Ok(k) => k,
            Err(_) => return fail("KEY_BINDING_MISMATCH", "invalid cnf.jwk", checks, dcount),
        };
        let kb_payload = match crypto::jws_verify(&holder_vk, kb_jwt) {
            Ok(p) => p,
            Err(_) => return fail("KEY_BINDING_MISMATCH", "KB-JWT signature invalid", checks, dcount),
        };
        let kb_aud = kb_payload.get("aud").and_then(|v| v.as_str()).unwrap_or("");
        if kb_aud != challenge.audience {
            return fail("KEY_BINDING_MISMATCH", "KB-JWT aud mismatch", checks, dcount);
        }
        // sd_hash over the presented SD-JWT + disclosures (everything before KB, incl. trailing ~).
        let presented = format!("{}~", parts[..parts.len() - 1].join("~"));
        let expected_sd_hash = crypto::sha256_b64url(presented.as_bytes());
        let kb_sd_hash = kb_payload.get("sd_hash").and_then(|v| v.as_str()).unwrap_or("");
        if kb_sd_hash != expected_sd_hash {
            return fail("KEY_BINDING_MISMATCH", "KB-JWT sd_hash mismatch", checks, dcount);
        }
        checks.push(CheckResult::pass("KEY_BINDING", "KB-JWT signature, aud, sd_hash valid"));

        // (e) Nonce binds to this request (single-use consumption in the handler).
        let kb_nonce = kb_payload.get("nonce").and_then(|v| v.as_str()).unwrap_or("");
        if kb_nonce != challenge.nonce {
            return fail("NONCE_MISMATCH", "KB-JWT nonce != request nonce", checks, dcount);
        }
        checks.push(CheckResult::pass("NONCE_BINDING", "KB-JWT nonce matches request"));

        // (f) Derive evidence from disclosures against the injected clock.
        let verified_evidence = derive_evidence(&challenge.required_evidence, &disclosed, now);
        checks.push(CheckResult::pass(
            "CLAIM_DERIVATION",
            "derived boolean claims (raw values discarded)",
        ));

        VerificationResult {
            ok: true,
            failed_check: None,
            checks,
            verified_evidence,
            nonce: kb_nonce.to_string(),
            audience: kb_aud.to_string(),
            disclosure_count: dcount,
        }
    }
}

/// Map required evidence types to the SD-JWT claim that establishes them and
/// derive the evidence STATE (never the raw value). Missing disclosure => MISSING
/// (=> policy UNKNOWN); an affirmatively false derived claim => INVALID (=> DENY).
fn derive_evidence(
    required: &[String],
    disclosed: &BTreeMap<String, Value>,
    now: OffsetDateTime,
) -> Vec<Evidence> {
    let mut out = Vec::new();
    for et in required {
        let state = match required_claim_for(et) {
            Some(claim) => match disclosed.get(claim) {
                None => EvidenceState::Missing,
                Some(value) => derive_state(et, value, now),
            },
            None => EvidenceState::Missing,
        };
        out.push(Evidence { evidence_type: et.clone(), state });
    }
    out
}

fn required_claim_for(evidence_type: &str) -> Option<&'static str> {
    match evidence_type {
        "AGE_ATTESTATION" => Some("birth_date"),
        "GOVERNMENT_IDENTITY" => Some("family_name"),
        "RESIDENCE_CREDENTIAL" => Some("resident_country"),
        _ => None,
    }
}

const EU_COUNTRIES: &[&str] = &[
    "AT", "BE", "BG", "HR", "CY", "CZ", "DK", "EE", "FI", "FR", "DE", "GR", "HU", "IE", "IT", "LV",
    "LT", "LU", "MT", "NL", "PL", "PT", "RO", "SK", "SI", "ES", "SE",
];

fn derive_state(evidence_type: &str, value: &Value, now: OffsetDateTime) -> EvidenceState {
    match evidence_type {
        "AGE_ATTESTATION" => match value.as_str().map(|s| is_over_18(s, now)) {
            Some(Ok(true)) => EvidenceState::Available,
            Some(Ok(false)) => EvidenceState::Invalid, // affirmatively under 18 -> DENY
            _ => EvidenceState::Unknown,
        },
        "RESIDENCE_CREDENTIAL" => match value.as_str() {
            Some(c) if EU_COUNTRIES.contains(&c) => EvidenceState::Available,
            Some(_) => EvidenceState::Invalid, // affirmatively non-EU
            None => EvidenceState::Unknown,
        },
        // Presence of a valid identity claim establishes the evidence.
        _ => EvidenceState::Available,
    }
}

/// Age check against the injected clock — deterministic.
fn is_over_18(birth_date: &str, now: OffsetDateTime) -> Result<bool, String> {
    let fmt = time::macros::format_description!("[year]-[month]-[day]");
    let bd = time::Date::parse(birth_date, &fmt).map_err(|e| format!("birth_date: {e}"))?;
    let today = now.date();
    let mut years = today.year() - bd.year();
    let bm = u8::from(bd.month());
    let tm = u8::from(today.month());
    if (tm, today.day()) < (bm, bd.day()) {
        years -= 1;
    }
    Ok(years >= 18)
}
