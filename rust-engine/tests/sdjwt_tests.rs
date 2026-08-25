//! Real SD-JWT VC verification tests: happy path + every tamper/failure mode +
//! determinism. A tiny in-test "wallet" builds VPs so we exercise the exact wire
//! format the Java wallet must match.

use engine::config::Registry;
use engine::crypto;
use engine::domain::{Decision, Kind};
use engine::evidence::{CredentialVerifier, Presentation, SdJwtCredentialVerifier};
use engine::issuer::{IssuedCredential, Issuer};
use engine::policy;
use engine::replay::Challenge;
use p256::ecdsa::SigningKey;
use serde_json::json;
use time::macros::datetime;
use time::{Duration, OffsetDateTime};

fn registry() -> Registry {
    Registry::load(&std::path::PathBuf::from("config")).expect("config")
}

fn issue_now() -> OffsetDateTime {
    datetime!(2025-06-01 00:00:00 UTC)
}

/// In-test wallet: assemble a VP = issuer-jwt ~ selected-disclosures ~ kb-jwt.
fn build_vp(
    issued: &IssuedCredential,
    holder: &SigningKey,
    select: &[&str],
    nonce: &str,
    aud: &str,
    now: OffsetDateTime,
) -> String {
    let mut presented = issued.sd_jwt.clone();
    for d in &issued.disclosures {
        if select.contains(&d.claim.as_str()) {
            presented.push('~');
            presented.push_str(&d.disclosure);
        }
    }
    presented.push('~');
    let sd_hash = crypto::sha256_b64url(presented.as_bytes());
    let kb_header = json!({ "alg": "ES256", "typ": "kb+jwt" });
    let kb_payload = json!({
        "iat": now.unix_timestamp(),
        "aud": aud,
        "nonce": nonce,
        "sd_hash": sd_hash,
    });
    let kb = crypto::jws_sign(holder, &kb_header, &kb_payload);
    format!("{presented}{kb}")
}

fn challenge(nonce: &str, aud: &str) -> Challenge {
    let now = OffsetDateTime::now_utc();
    Challenge {
        request_id: "req-1".into(),
        nonce: nonce.into(),
        audience: aud.into(),
        issued_at: now,
        expires_at: now + Duration::seconds(300),
        canonical: "age_over_18".into(),
        kind: Kind::Predicate,
        subject_id: "user-123".into(),
        required_evidence: vec!["AGE_ATTESTATION".into()],
    }
}

fn pres(vp: String) -> Presentation {
    Presentation {
        sd_jwt_vp: Some(vp),
        ..Default::default()
    }
}

struct Fixture {
    issuer: Issuer,
    holder: SigningKey,
    issued: IssuedCredential,
    verifier: SdJwtCredentialVerifier,
}

fn setup() -> Fixture {
    let issuer = Issuer::new();
    let holder = crypto::generate_key();
    let holder_jwk = crypto::jwk_from_verifying(holder.verifying_key());
    let issued = issuer.issue(&engine::issuer::demo_claims("user-123"), holder_jwk, issue_now());
    let verifier = SdJwtCredentialVerifier::new(issuer.verifying_key(), issuer.iss.clone());
    Fixture { issuer, holder, issued, verifier }
}

fn flip_last(s: &str) -> String {
    let mut c: Vec<char> = s.chars().collect();
    let i = c.len() - 1;
    c[i] = if c[i] == 'A' { 'B' } else { 'A' };
    c.into_iter().collect()
}

const AUD: &str = "relying-party-demo";

#[test]
fn happy_path_minimal_disclosure_allows_and_hides_dob() {
    let f = setup();
    let vp = build_vp(&f.issued, &f.holder, &["birth_date"], "nonce-1", AUD, issue_now());
    let vr = f.verifier.verify(&pres(vp), &challenge("nonce-1", AUD), issue_now());

    assert!(vr.ok, "checks: {:?}", vr.checks);
    assert_eq!(vr.disclosure_count, 1, "only birth_date disclosed");
    assert!(vr.failed_check.is_none());

    let report = policy::evaluate(
        &registry(),
        "age_over_18",
        Kind::Predicate,
        &vr.verified_evidence,
        issue_now(),
    );
    assert_eq!(report.decision, Decision::Allow);
    assert_eq!(report.verified_claims.get("age_over_18"), Some(&true));

    // No raw attribute VALUES anywhere in the decision body.
    let body = serde_json::to_string(&report).unwrap().to_lowercase();
    assert!(!body.contains("birth_date"));
    assert!(!body.contains("1984"));
    assert!(!body.contains("erika"));
    assert!(!body.contains("mustermann"));
}

#[test]
fn signature_tamper_is_rejected() {
    let f = setup();
    let parts: Vec<&str> = f.issued.sd_jwt.split('.').collect();
    let tampered_jwt = format!("{}.{}.{}", parts[0], parts[1], flip_last(parts[2]));
    let issued2 = IssuedCredential { sd_jwt: tampered_jwt, ..f.issued.clone() };
    let vp = build_vp(&issued2, &f.holder, &["birth_date"], "nonce-1", AUD, issue_now());
    let vr = f.verifier.verify(&pres(vp), &challenge("nonce-1", AUD), issue_now());
    assert!(!vr.ok);
    assert_eq!(vr.failed_check.as_deref(), Some("INVALID_SIGNATURE"));
}

#[test]
fn disclosure_tamper_is_rejected() {
    let f = setup();
    let mut issued3 = f.issued.clone();
    for d in issued3.disclosures.iter_mut() {
        if d.claim == "birth_date" {
            d.disclosure = flip_last(&d.disclosure);
        }
    }
    let vp = build_vp(&issued3, &f.holder, &["birth_date"], "nonce-1", AUD, issue_now());
    let vr = f.verifier.verify(&pres(vp), &challenge("nonce-1", AUD), issue_now());
    assert!(!vr.ok);
    assert_eq!(vr.failed_check.as_deref(), Some("UNKNOWN_DISCLOSURE"));
}

#[test]
fn wrong_nonce_is_rejected() {
    let f = setup();
    let vp = build_vp(&f.issued, &f.holder, &["birth_date"], "attacker-nonce", AUD, issue_now());
    let vr = f.verifier.verify(&pres(vp), &challenge("nonce-1", AUD), issue_now());
    assert!(!vr.ok);
    assert_eq!(vr.failed_check.as_deref(), Some("NONCE_MISMATCH"));
}

#[test]
fn expired_credential_is_rejected() {
    let f = setup();
    let vp = build_vp(&f.issued, &f.holder, &["birth_date"], "nonce-1", AUD, issue_now());
    let later = issue_now() + Duration::hours(48); // past 24h TTL
    let vr = f.verifier.verify(&pres(vp), &challenge("nonce-1", AUD), later);
    assert!(!vr.ok);
    assert_eq!(vr.failed_check.as_deref(), Some("EXPIRED_CREDENTIAL"));
}

#[test]
fn key_binding_with_wrong_key_is_rejected() {
    let f = setup();
    let attacker = crypto::generate_key(); // not the cnf key
    let vp = build_vp(&f.issued, &attacker, &["birth_date"], "nonce-1", AUD, issue_now());
    let vr = f.verifier.verify(&pres(vp), &challenge("nonce-1", AUD), issue_now());
    assert!(!vr.ok);
    assert_eq!(vr.failed_check.as_deref(), Some("KEY_BINDING_MISMATCH"));
}

#[test]
fn wrong_audience_is_rejected() {
    let f = setup();
    let vp = build_vp(&f.issued, &f.holder, &["birth_date"], "nonce-1", "someone-else", issue_now());
    let vr = f.verifier.verify(&pres(vp), &challenge("nonce-1", AUD), issue_now());
    assert!(!vr.ok);
    assert_eq!(vr.failed_check.as_deref(), Some("KEY_BINDING_MISMATCH"));
}

#[test]
fn determinism_same_vp_same_clock_byte_identical() {
    let f = setup();
    let vp = build_vp(&f.issued, &f.holder, &["birth_date"], "nonce-1", AUD, issue_now());
    let ch = challenge("nonce-1", AUD);

    let vr1 = f.verifier.verify(&pres(vp.clone()), &ch, issue_now());
    let vr2 = f.verifier.verify(&pres(vp), &ch, issue_now());
    let reg = registry();
    let r1 = policy::evaluate(&reg, "age_over_18", Kind::Predicate, &vr1.verified_evidence, issue_now());
    let r2 = policy::evaluate(&reg, "age_over_18", Kind::Predicate, &vr2.verified_evidence, issue_now());
    assert_eq!(
        serde_json::to_string(&r1).unwrap(),
        serde_json::to_string(&r2).unwrap()
    );
}

#[test]
fn jwks_exposes_issuer_public_key() {
    let f = setup();
    let jwks = f.issuer.jwks();
    let k = &jwks["keys"][0];
    assert_eq!(k["kty"], "EC");
    assert_eq!(k["crv"], "P-256");
    assert_eq!(k["alg"], "ES256");
    assert!(k["x"].is_string() && k["y"].is_string());
}
