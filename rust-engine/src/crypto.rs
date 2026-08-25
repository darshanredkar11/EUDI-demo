//! Real ES256 (P-256) primitives for SD-JWT VC: compact JWS sign/verify, JWK
//! conversion, SHA-256, base64url. No fake algorithms — this is RustCrypto.
//!
//! A production system would add issuer trust lists, key rotation, and
//! algorithm agility; here a single ES256 key per role is sufficient and honest.

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine;
use p256::ecdsa::signature::{Signer, Verifier};
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use p256::{EncodedPoint, FieldBytes};
use rand_core::OsRng;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub fn b64url(bytes: &[u8]) -> String {
    B64.encode(bytes)
}

pub fn b64url_decode(s: &str) -> Result<Vec<u8>, String> {
    B64.decode(s.as_bytes()).map_err(|e| format!("base64url: {e}"))
}

pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

pub fn sha256_b64url(bytes: &[u8]) -> String {
    b64url(&sha256(bytes))
}

/// Generate a fresh ES256 signing key (randomness is confined here — it never
/// enters a decision body).
pub fn generate_key() -> SigningKey {
    SigningKey::random(&mut OsRng)
}

/// Public JWK ({kty,crv,x,y}) for a verifying key.
pub fn jwk_from_verifying(vk: &VerifyingKey) -> Value {
    let ep = vk.to_encoded_point(false);
    let x = ep.x().expect("P-256 has x");
    let y = ep.y().expect("uncompressed point has y");
    json!({
        "kty": "EC",
        "crv": "P-256",
        "x": b64url(x),
        "y": b64url(y),
    })
}

/// Reconstruct a verifying key from an EC P-256 JWK.
pub fn verifying_from_jwk(jwk: &Value) -> Result<VerifyingKey, String> {
    let x = jwk.get("x").and_then(|v| v.as_str()).ok_or("jwk missing x")?;
    let y = jwk.get("y").and_then(|v| v.as_str()).ok_or("jwk missing y")?;
    let xb = b64url_decode(x)?;
    let yb = b64url_decode(y)?;
    if xb.len() != 32 || yb.len() != 32 {
        return Err("jwk coordinate length != 32".to_string());
    }
    let ep = EncodedPoint::from_affine_coordinates(
        FieldBytes::from_slice(&xb),
        FieldBytes::from_slice(&yb),
        false,
    );
    let vk = VerifyingKey::from_encoded_point(&ep)
        .map_err(|_| "invalid P-256 point in jwk".to_string())?;
    Ok(vk)
}

/// Sign a compact JWS (ES256): b64url(header).b64url(payload).b64url(sig).
pub fn jws_sign(sk: &SigningKey, header: &Value, payload: &Value) -> String {
    let h = b64url(header.to_string().as_bytes());
    let p = b64url(payload.to_string().as_bytes());
    let signing_input = format!("{h}.{p}");
    let sig: Signature = sk.sign(signing_input.as_bytes());
    let s = b64url(&sig.to_bytes());
    format!("{signing_input}.{s}")
}

/// Verify a compact JWS against a verifying key; return the decoded payload.
pub fn jws_verify(vk: &VerifyingKey, jwt: &str) -> Result<Value, String> {
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err("jws: expected 3 parts".to_string());
    }
    let signing_input = format!("{}.{}", parts[0], parts[1]);
    let sig_bytes = b64url_decode(parts[2])?;
    let sig = Signature::from_slice(&sig_bytes).map_err(|e| format!("jws sig: {e}"))?;
    vk.verify(signing_input.as_bytes(), &sig)
        .map_err(|_| "jws: signature verification failed".to_string())?;
    let payload_bytes = b64url_decode(parts[1])?;
    serde_json::from_slice(&payload_bytes).map_err(|e| format!("jws payload: {e}"))
}

/// Decode a JWS payload WITHOUT verifying (for reading a KB-JWT's cnf-bound
/// fields before we have selected the key). Never trust these until verified.
pub fn jws_payload_unverified(jwt: &str) -> Result<Value, String> {
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err("jws: expected 3 parts".to_string());
    }
    let payload_bytes = b64url_decode(parts[1])?;
    serde_json::from_slice(&payload_bytes).map_err(|e| format!("jws payload: {e}"))
}
