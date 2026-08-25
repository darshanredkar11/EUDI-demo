//! Hand-curated OpenAPI document for the engine contract, served with a bundled
//! Swagger UI (utoipa-swagger-ui). Every route carries a description and a
//! realistic example so the whole flow is runnable from the browser.

use serde_json::{json, Value};

/// The engine's OpenAPI 3.0 document as raw JSON (served at
/// /api-docs/openapi.json and loaded by the bundled Swagger UI).
pub fn openapi_json() -> Value {
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Identity Evidence Engine (Rust)",
            "version": "0.2.0",
            "description": "Deterministic decision core: question registry + proposal validation, \
reverse evidence inference, policy evaluation, REAL SD-JWT VC issuance & verification, \
nonce/replay protection, and audit. No AI runs here — the LLM proposes in the Java layer; \
the engine disposes."
        },
        "servers": [{ "url": "http://localhost:8081" }],
        "tags": [
            { "name": "1-Question Resolution", "description": "Tier-1 registry + Tier-2 proposal validation" },
            { "name": "3-Presentation & Verification", "description": "Plan, evaluate, verify SD-JWT presentations" },
            { "name": "4-Audit", "description": "Provenance (types + check results, never PII)" },
            { "name": "5-Issuer", "description": "Demo PID issuer (ES256 SD-JWT VC) + JWKS" },
            { "name": "Health", "description": "Liveness" }
        ],
        "paths": {
            "/health": {
                "get": {
                    "tags": ["Health"],
                    "summary": "Liveness probe",
                    "responses": { "200": { "description": "ok" } }
                }
            },
            "/engine/resolve": {
                "post": {
                    "tags": ["1-Question Resolution"],
                    "summary": "Resolve a question to a canonical predicate/policy",
                    "description": "Tier 1 exact-match on the registry. If `proposedCanonical` is supplied \
(a validated LLM proposal), the engine accepts it only if it is a member of the closed set.",
                    "requestBody": { "required": true, "content": { "application/json": {
                        "examples": {
                            "tier1": { "summary": "Registry hit", "value": { "question": "Is this user over 18?" } },
                            "tier2": { "summary": "Validate an LLM proposal", "value": { "question": "Is this customer an adult?", "proposedCanonical": "age_over_18" } }
                        }
                    } } },
                    "responses": {
                        "200": { "description": "Resolved", "content": { "application/json": { "example": {
                            "canonical": "age_over_18", "kind": "predicate", "resolvedBy": "REGISTRY" } } } },
                        "422": { "description": "Unresolved", "content": { "application/json": { "example": {
                            "error": "UNRESOLVED_QUESTION", "supportedQuestions": ["BANK_ACCOUNT_OPENING_V1", "age_over_18"] } } } }
                    }
                }
            },
            "/engine/validate-canonical": {
                "post": {
                    "tags": ["1-Question Resolution"],
                    "summary": "Validate a proposed canonical id against the closed set",
                    "requestBody": { "required": true, "content": { "application/json": { "example": { "proposedCanonical": "age_over_18" } } } },
                    "responses": {
                        "200": { "description": "Valid member", "content": { "application/json": { "example": { "canonical": "age_over_18", "kind": "predicate", "resolvedBy": "LLM_VALIDATED" } } } },
                        "422": { "description": "Not a member" }
                    }
                }
            },
            "/engine/plan": {
                "post": {
                    "tags": ["3-Presentation & Verification"],
                    "summary": "Reverse inference: required evidence + a nonce challenge",
                    "requestBody": { "required": true, "content": { "application/json": { "example": {
                        "canonical": "age_over_18", "subjectId": "user-123", "knownEvidence": [] } } } },
                    "responses": { "200": { "description": "Plan + challenge", "content": { "application/json": { "example": {
                        "requestId": "b1c2…", "canonical": "age_over_18", "kind": "predicate",
                        "requiredEvidence": ["AGE_ATTESTATION"], "nonce": "de7d…", "expiresAt": "2026-01-01T00:05:00Z" } } } } }
                }
            },
            "/engine/evaluate": {
                "post": {
                    "tags": ["3-Presentation & Verification"],
                    "summary": "Deterministic policy evaluation (injected clock)",
                    "requestBody": { "required": true, "content": { "application/json": { "example": {
                        "canonical": "age_over_18",
                        "evidence": [{ "type": "AGE_ATTESTATION", "state": "AVAILABLE" }],
                        "evaluationTime": "2025-06-01T00:00:00Z" } } } },
                    "responses": { "200": { "description": "Decision", "content": { "application/json": { "example": {
                        "decision": "ALLOW", "canonical": "age_over_18", "verifiedClaims": { "age_over_18": true },
                        "auditId": "ec92…" } } } } }
                }
            },
            "/engine/presentations": {
                "post": {
                    "tags": ["3-Presentation & Verification"],
                    "summary": "Verify an SD-JWT VP (real crypto) then evaluate",
                    "description": "Checks: issuer signature, disclosure digests, credential validity, key binding, \
nonce; single-use nonce consumed on success (replay → REPLAY_DETECTED). Raw attribute values never leave the engine.",
                    "requestBody": { "required": true, "content": { "application/json": { "example": {
                        "requestId": "b1c2…",
                        "presentation": { "sdJwtVp": "<issuer-jwt>~<birth_date-disclosure>~<kb-jwt>" } } } } },
                    "responses": { "200": { "description": "Decision + verification checks", "content": { "application/json": {
                        "examples": {
                            "allow": { "summary": "ALLOW", "value": { "decision": "ALLOW", "predicate": "age_over_18",
                                "verifiedClaims": { "age_over_18": true }, "disclosureCount": 1,
                                "verificationChecks": [{ "name": "ISSUER_SIGNATURE", "ok": true }], "auditId": "ec92…" } },
                            "replay": { "summary": "REPLAY_DETECTED", "value": { "decision": "REPLAY_DETECTED", "reason": "NONCE_ALREADY_CONSUMED", "auditId": "4ea2…" } },
                            "deny": { "summary": "DENY (failed check)", "value": { "decision": "DENY", "failedCheck": "UNKNOWN_DISCLOSURE",
                                "verificationChecks": [{ "name": "UNKNOWN_DISCLOSURE", "ok": false }], "auditId": "9f0a…" } }
                        }
                    } } } }
                }
            },
            "/engine/issuer/issue": {
                "post": {
                    "tags": ["5-Issuer"],
                    "summary": "Issue a PID SD-JWT VC bound to a holder key (cnf)",
                    "requestBody": { "required": true, "content": { "application/json": { "example": {
                        "subjectId": "user-123",
                        "holderJwk": { "kty": "EC", "crv": "P-256", "x": "…", "y": "…" } } } } },
                    "responses": { "200": { "description": "Issued SD-JWT VC", "content": { "application/json": { "example": {
                        "sdJwt": "eyJhbGciOiJFUzI1Ni…", "disclosures": [{ "claim": "birth_date", "disclosure": "WyJ…" }],
                        "vct": "urn:eu.europa.ec.eudi:pid:1", "expiresAt": "2026-01-02T00:00:00Z" } } } } }
                }
            },
            "/engine/issuer/jwks": {
                "get": {
                    "tags": ["5-Issuer"],
                    "summary": "Issuer public keys (trust-list stand-in)",
                    "responses": { "200": { "description": "JWKS", "content": { "application/json": { "example": {
                        "keys": [{ "kty": "EC", "crv": "P-256", "x": "…", "y": "…", "kid": "demo-issuer-key-1", "use": "sig", "alg": "ES256" }] } } } } }
                }
            },
            "/engine/audits/{id}": {
                "get": {
                    "tags": ["4-Audit"],
                    "summary": "Fetch an audit record by id",
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "responses": {
                        "200": { "description": "Audit", "content": { "application/json": { "example": {
                            "auditId": "ec92…", "canonical": "age_over_18", "decision": "ALLOW",
                            "predicates": { "age_over_18": "TRUE" }, "disclosureCount": 1,
                            "verificationChecks": [{ "name": "KEY_BINDING", "ok": true }] } } } },
                        "404": { "description": "Not found" }
                    }
                }
            }
        }
    })
}
