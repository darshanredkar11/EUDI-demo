//! Demo PID issuer. Holds a static ES256 keypair (generated at startup) and
//! issues IETF SD-JWT VCs: an issuer-signed JWT carrying `_sd` digests, plus one
//! disclosure per selectively-disclosable claim. Key binding (`cnf`) binds the
//! credential to the holder's public key.
//!
//! The issuer's public key (published at /engine/issuer/jwks) stands in for a
//! trust list in this demo — a real deployment resolves issuer keys from an
//! eIDAS/EUDI trust list. Everything cryptographic here is REAL.

use crate::crypto;
use p256::ecdsa::{SigningKey, VerifyingKey};
use serde::Serialize;
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

/// Credential validity for the demo (24h).
pub const CREDENTIAL_TTL_SECONDS: i64 = 24 * 3600;
pub const VCT: &str = "urn:eu.europa.ec.eudi:pid:1";

pub struct Issuer {
    signing: SigningKey,
    pub kid: String,
    pub iss: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DisclosureOut {
    pub claim: String,
    /// base64url(JSON [salt, name, value]) — the wallet stores these.
    pub disclosure: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IssuedCredential {
    /// The issuer-signed JWT (no disclosures appended).
    #[serde(rename = "sdJwt")]
    pub sd_jwt: String,
    /// One entry per selectively-disclosable claim.
    pub disclosures: Vec<DisclosureOut>,
    /// Convenience: issuer JWT + all disclosures, `~`-joined, trailing `~`.
    pub combined: String,
    pub vct: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: String,
}

impl Issuer {
    pub fn new() -> Self {
        Self {
            signing: crypto::generate_key(),
            kid: "demo-issuer-key-1".to_string(),
            iss: "https://issuer.eudi-demo.local".to_string(),
        }
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        *self.signing.verifying_key()
    }

    /// JWKS document for /engine/issuer/jwks.
    pub fn jwks(&self) -> Value {
        let mut jwk = crypto::jwk_from_verifying(&self.verifying_key());
        let obj = jwk.as_object_mut().unwrap();
        obj.insert("kid".to_string(), json!(self.kid));
        obj.insert("use".to_string(), json!("sig"));
        obj.insert("alg".to_string(), json!("ES256"));
        json!({ "keys": [jwk] })
    }

    /// Issue an SD-JWT VC. `claims` are the (name,value) attributes; EACH is
    /// selectively disclosable behind its own salted `_sd` digest. `holder_jwk`
    /// is bound via `cnf` for key binding.
    pub fn issue(
        &self,
        claims: &[(String, Value)],
        holder_jwk: Value,
        now: OffsetDateTime,
    ) -> IssuedCredential {
        let iat = now.unix_timestamp();
        let exp_dt = now + Duration::seconds(CREDENTIAL_TTL_SECONDS);
        let exp = exp_dt.unix_timestamp();

        let mut disclosures: Vec<DisclosureOut> = Vec::new();
        let mut digests: Vec<String> = Vec::new();
        for (name, value) in claims {
            // Disclosure = [ salt, claim_name, claim_value ]. Salt is random
            // (allowed nondeterminism — it never enters a decision body).
            let salt = crypto::b64url(&rand_salt());
            let arr = json!([salt, name, value]);
            let disclosure_b64 = crypto::b64url(arr.to_string().as_bytes());
            let digest = crypto::sha256_b64url(disclosure_b64.as_bytes());
            digests.push(digest);
            disclosures.push(DisclosureOut {
                claim: name.clone(),
                disclosure: disclosure_b64,
            });
        }
        // Sort digests so the issuer JWT does not leak claim order.
        digests.sort();

        let payload = json!({
            "iss": self.iss,
            "iat": iat,
            "exp": exp,
            "vct": VCT,
            "_sd_alg": "sha-256",
            "cnf": { "jwk": holder_jwk },
            "_sd": digests,
        });
        let header = json!({ "alg": "ES256", "typ": "dc+sd-jwt", "kid": self.kid });
        let sd_jwt = crypto::jws_sign(&self.signing, &header, &payload);

        let mut combined = sd_jwt.clone();
        for d in &disclosures {
            combined.push('~');
            combined.push_str(&d.disclosure);
        }
        combined.push('~');

        IssuedCredential {
            sd_jwt,
            disclosures,
            combined,
            vct: VCT.to_string(),
            expires_at: exp_dt.format(&Rfc3339).unwrap_or_default(),
        }
    }
}

impl Default for Issuer {
    fn default() -> Self {
        Self::new()
    }
}

/// Demo PID attribute fixture keyed by subject. All values are selectively
/// disclosable; nothing here is exposed unless the holder discloses it.
pub fn demo_claims(_subject_id: &str) -> Vec<(String, Value)> {
    vec![
        ("given_name".to_string(), json!("Erika")),
        ("family_name".to_string(), json!("Mustermann")),
        ("birth_date".to_string(), json!("1984-01-26")),
        ("resident_country".to_string(), json!("DE")),
    ]
}

fn rand_salt() -> [u8; 16] {
    use rand_core::RngCore;
    let mut b = [0u8; 16];
    rand_core::OsRng.fill_bytes(&mut b);
    b
}
