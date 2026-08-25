# Reverse Evidence Inference for Relying-Party Questions

### A deterministic decision core for the EUDI wallet era — technical white paper

**Status: working proof-of-concept (POC / demo).** This document describes a
running two-service prototype, not a product, and not a certified EUDI component.
Every claim below is scoped to what the code in this repository actually does.
The explicit boundary between what is *real*, what is *mocked*, and what is
*designed for future integration* is stated in §10, and nothing in the earlier
sections should be read as overriding it.

---

## Abstract

Most identity systems receive a Verifiable Credential (VC) and check a
signature. That answers "is this credential authentic?" but not the question a
relying party actually asked — *"Can this user open a bank account?"* or *"Is
this user over 18?"*. This POC inverts the usual flow: it takes a natural-language
business **question**, deterministically resolves it to a canonical predicate or
policy, runs **reverse inference** to compute exactly which evidence would prove
it (privacy-ranked, minimum first), issues a nonce-bound presentation request,
verifies a **real SD-JWT VC** presentation with genuine ES256 cryptography,
derives only the boolean claim the policy needs (the raw date of birth never
leaves the verifier), and returns a **deterministic** `ALLOW` / `DENY` /
`UNKNOWN` decision with a PII-free audit trail. A natural-language question is
optionally interpreted by an LLM, but the model is an *untrusted proposer* held
outside the decision core behind six enforced guardrails: **the LLM proposes;
the engine disposes.** The result is a design where AI improves usability at the
edge while the evidence → verification → decision chain stays deterministic,
auditable, and reproducible.

---

## 1. Problem and motivation

### 1.1 The relying party asks a business question, not a credential question

A bank onboarding flow, an age-gated shop, or a public service does not natively
care which credential a person holds. It has a *question*: may this person do
this thing? Framing identity around credential verification skips the
interesting, and privacy-critical, middle: **given a human question, decide what
must be proven, and disclose as little as possible to prove it.**

### 1.2 Why checking a VC signature is not enough

Verifying that a VC is authentically signed by a trusted issuer is necessary but
insufficient:

- It says nothing about **what must be true** for the relying party's specific
  question. A validly signed identity credential does not, by itself, answer
  "is this person an EU resident eligible to open an account?".
- It encourages **over-disclosure**. The simplest way to "prove age" with a full
  identity credential is to hand over the date of birth (and often name and
  address with it). That is exactly the data-minimization failure the EUDI
  architecture is meant to prevent.
- It has no first-class notion of **"I cannot tell yet."** A signature check is
  binary (valid / invalid); a real onboarding decision needs a third state for
  *missing or stale evidence*.

### 1.3 Minimal disclosure as a first-order requirement

The correct answer to "is this user over 18?" is a single boolean —
`age_over_18 = true` — derived from, but never accompanied by, the underlying
date of birth. This POC treats minimal disclosure as a property to be *enforced
structurally*, not merely encouraged: the wallet discloses only the claims the
request needs, and the verifier derives the boolean and discards the raw value
before any decision or audit record is produced (§6).

### 1.4 UNKNOWN must never silently become a wrong answer

When required evidence is missing, expired, or unresolved, the system must say
so — not guess, not default to `DENY`, and not silently fall through to `ALLOW`.
`UNKNOWN` is therefore a **first-class decision**, not an error and not a
degraded `DENY`. `DENY` is reserved for the case where evidence *affirmatively*
establishes falsity (e.g. an age claim that resolves to under 18, or a
non-EU residence value). Everything that merely *cannot be established* returns
`UNKNOWN`, typically accompanied by an evidence request plan describing what
would be needed to move forward.

---

## 2. Design principles

These principles are inherited from the philosophy of a Graph-based Inference
Engine (GIE) and run consistently through the codebase.

1. **Deterministic decision core.** All identity and policy logic lives in the
   Rust engine. There is no AI dependency inside the engine; the decision and
   evaluation paths contain zero LLM calls (guardrail G4). The architecture
   makes the trust boundary visible.

2. **Explicit facts and states.** Evidence is modelled as a `type` plus an
   explicit `state` (`AVAILABLE`, `MISSING`, `EXPIRED`, `INVALID`, `REVOKED`,
   `UNKNOWN`). Predicates resolve to `TRUE` / `FALSE` / `UNKNOWN`. Nothing is
   implicit.

3. **First-class UNKNOWN.** Missing or stale information yields `UNKNOWN`, never
   a silently wrong `ALLOW`/`DENY` (see `policy/mod.rs`: "UNKNOWN is a
   legitimate first-class decision, never an error").

4. **Full provenance.** Every decision — including `UNKNOWN`, `REPLAY_DETECTED`,
   and `UNRESOLVED_QUESTION` — emits an audit object that explains *why* using
   evidence **types and states only, never raw attribute values** (`audit/mod.rs`).

5. **Same input + same state ⇒ same result.** Evaluation uses an injected clock,
   sorted JSON output, and config-ordered arrays. The only non-determinism in
   the whole system is cryptographic randomness (keys, salts, nonces), which is
   confined to issuance/challenge generation and never enters a decision body
   (§8).

---

## 3. The reverse-inference thesis

### 3.1 GIE forward vs. this system's reverse

A Graph-based Inference Engine runs *forward*: it accumulates knowledge and
infers an answer or recommendation. This system runs the **same deterministic
machinery in reverse**:

| | direction |
|---|---|
| **GIE** | knowledge → inference → answer / recommendation |
| **This system** | question → required predicates → required evidence → verification → answer |

The reverse direction is why the engine's modules are named `resolution/`
(question → canonical) and `inference/` (canonical → predicates → evidence). The
inference module's own header calls this "the centerpiece": it goes
*"canonical question/policy → required predicates → concrete evidence types
(privacy-ranked) → only what is still MISSING given current subject state."*

The full pipeline is:

> **QUESTION → WHAT MUST BE TRUE? → WHAT EVIDENCE PROVES IT? → WHAT IS THE
> MINIMUM INFORMATION NEEDED? → REQUEST → VERIFY → DETERMINISTIC DECISION**

### 3.2 Architecture

```
                    Business Question (natural language)
                                   │
                                   ▼
┌───────────── Java Spring Boot :8080 — thin API + orchestration ──────────────┐
│  VerificationController / WalletController / AuditController                   │
│      → VerificationService  (orchestration only; NO business/policy logic)    │
│            ├─ QuestionResolver  (Tier 2: StubLlmResolver | Anthropic…)  ◄─ UNTRUSTED
│            │      sees ONLY {question, catalogue};  never PII/VC/VP  (G3)      │
│            ├─ EngineClient  (RestClient over HTTP JSON, explicit DTOs)         │
│            └─ MockWallet  (real ES256 holder key, SD-JWT VP, KB-JWT)           │
└───────────────────────────────────┬──────────────────────────────────────────┘
                                     │  HTTP JSON  (explicit DTOs both sides)
                                     ▼
┌────────── Rust axum engine :8081 — deterministic decision core ──────────────┐
│                                                                               │
│   resolution/   Tier 1 registry (normalize+alias) + LLM-proposal validation  │
│        │                                                                      │
│        ▼                                                                      │
│   inference/    REVERSE: canonical → predicates → evidence (privacy-ranked)   │
│        │              build_plan(): only what is still MISSING (minimize)     │
│        ▼                                                                      │
│   replay/       challenge: single-use 256-bit nonce + TTL (ReplayStore seam)  │
│        │                                                                      │
│        ▼                                                                      │
│   issuer/       demo PID issuer: SD-JWT VC, salted _sd digests, cnf binding   │
│        │              JWKS at /engine/issuer/jwks  (trust-list STAND-IN)      │
│        ▼                                                                      │
│   evidence/     CredentialVerifier seam:                                      │
│        │            SdJwtCredentialVerifier (REAL ES256)  ← live              │
│        │            MockCredentialVerifier (tests only)                       │
│        │        six named checks → derive boolean claim (raw value discarded) │
│        ▼                                                                      │
│   policy/       evaluate → ALLOW | DENY | UNKNOWN   (first-class UNKNOWN)     │
│        │                                                                      │
│        ▼                                                                      │
│   audit/        provenance: types + states, resolvedBy, checks — NEVER PII    │
│                                                                               │
│   config/       questions.yaml · predicates.yaml · policies.yaml  (YAML KB)   │
└───────────────────────────────────────────────────────────────────────────────┘
```

**Boundary (non-negotiable):** all identity/policy business logic lives in Rust.
Java only translates the external API to the engine contract and orchestrates
two-tier question resolution. The trust boundary is legible in the architecture
itself: the untrusted component sits at the top edge and is physically unable to
reach the decision core's inputs.

---

## 4. Two-tier question resolution and the AI trust boundary

Resolution maps a human question to a **canonical predicate or policy id**. It is
two-tier, and the decision core stays 100% deterministic because **the LLM
PROPOSES, the engine DISPOSES.**

### 4.1 Tier 1 — deterministic registry (Rust, `config/questions.yaml`)

`resolution/mod.rs` implements exactly this algorithm:

1. **Normalize** the input: lowercase → trim → collapse internal whitespace →
   strip terminal punctuation (`?`, `.`, `!`).
2. **Exact-match** the normalized string against the normalized alias table.
3. If the input already **is** a canonical id (e.g. `age_over_18` or
   `BANK_ACCOUNT_OPENING_V1`), accept it directly (case-sensitive canonical
   match).
4. No match → fall through to Tier 2.

The registry is knowledge, so it lives in the engine, not the orchestration
layer. Example aliases (from `questions.yaml`) include *"is this user over 18"*
and *"can this user buy alcohol"* both mapping to the `age_over_18` predicate,
and *"can this user open a bank account"* mapping to the
`BANK_ACCOUNT_OPENING_V1` policy.

### 4.2 Tier 2 — guardrailed LLM proposer (Java)

On a Tier-1 miss, an LLM is asked to *propose* one member of the closed canonical
set (or abstain). The default `StubLlmResolver` runs when no API key is present;
`AnthropicLlmResolver` (model `claude-sonnet-4-6`, temperature 0) runs when
`ANTHROPIC_API_KEY` is set. The guardrail pipeline is identical either way — only
the model call is substituted.

**Trust boundary (verbatim):**

> The LLM is an UNTRUSTED PROPOSER outside the decision core. It maps a
> paraphrased question to ONE member of a closed set of canonical
> predicates/policies, or ABSTAIN. Every proposal is validated against the
> deterministic registry before use. The LLM never sees credentials, VCs, VPs,
> subject attributes, or any PII. It never participates in evidence evaluation,
> verification, replay protection, or decisions.

### 4.3 Guardrails G1–G6 (all enforced, each demonstrable)

- **G1 — Closed-world output.** The prompt enumerates the canonical set with
  one-line descriptions and demands strict JSON:
  `{ "canonical": "<id>"|null, "confidence": "HIGH|MEDIUM|LOW", "reason": "<short>" }`.
  The system prompt forbids prose/markdown; the resolver extracts and parses the
  JSON object (`AnthropicLlmResolver.buildPrompt` / `extractJson`).
- **G2 — Two-part validation.** A proposal is accepted only if `confidence ==
  HIGH` **and** the `canonical` string exactly matches a registry entry.
  Confidence is gated in Java (`proposal.isHighConfidenceCandidate()`), and
  **membership is re-validated by the engine** via `POST /engine/resolve` with a
  `proposedCanonical` (`resolution::validate_proposal`, which returns a result
  only if `registry.kind_of(proposed)` is `Some`). The engine never trusts the
  LLM's string; anything else → `UNRESOLVED_QUESTION` (HTTP 422).
- **G3 — Data minimization for the AI itself.** The resolver receives **only**
  the raw question plus the catalogue (ids + descriptions). This is enforced
  *structurally*: the `QuestionResolver.propose(String question,
  List<CanonicalEntry> catalogue)` signature has no parameter for
  `subjectId`, credentials, VC/VP contents, or evidence — there is literally no
  channel through which PII could reach the model.
- **G4 — Deterministic core untouched.** A validated Tier-2 resolution feeds the
  *same* engine path as Tier 1. Temperature is 0. The decision/evaluation path
  contains zero LLM calls.
- **G5 — Auditability.** An LLM-assisted resolution records `resolvedBy:
  LLM_VALIDATED`, the model id, and the raw proposal JSON (`llmProposal`) in the
  audit (`audit::Audit.llm_proposal`). Every response also carries `resolvedBy`
  (`REGISTRY` or `LLM_VALIDATED`).
- **G6 — Fail closed.** LLM unavailable, timeout (3s), error, abstain, or any
  non-HIGH confidence → `UNRESOLVED_QUESTION`, never a guess and never a blocking
  retry loop. `AnthropicLlmResolver.propose` catches every exception and returns
  an ABSTAIN proposal.

### 4.4 Why guessing is impossible by construction

There are only two ways forward out of resolution: a **validated member of the
closed set**, or a **refusal**. A hallucinated id fails engine membership
re-validation (G2). A low-confidence guess fails the Java HIGH gate (G2/G6). An
outage fails closed (G6). And because the model can never see credential data
(G3, structural), even a compromised or adversarial model cannot leak PII or
influence a decision — it can only affect *which canonical question* is asked,
and even that is bounded to the closed set the engine will accept. This is the
correct EUDI+AI shape: AI improves usability at the edge (understanding a
paraphrase) while the ARF-style trust chain (evidence → verification → decision)
stays deterministic and auditable, and **data minimization is applied to the AI
component itself.**

---

## 4A. AI threat model

> **We never try to make the model behave; we make misbehavior harmless. Every AI
> attack collapses to a refusal (fail closed) or an outcome the attacker could
> have had by asking plainly. Integrity is never at stake — only availability of
> the paraphrase convenience path.**

The LLM is an untrusted proposer at the edge. The attack classes below are
neutralized *structurally* — by the shape of the architecture — not by prompt
hardening. OWASP LLM Top-10 ids are given for recognizability.

| Attack class (OWASP LLM) | Neutralizing mechanism | Residual risk |
|---|---|---|
| **Prompt injection** (LLM01) | Confinement: the only usable output is *one member of a closed set*, re-validated engine-side at Gate 2. The attacker controls only their own question, so a steered mapping merely answers a *different in-set* question — it cannot mint a new predicate, skip evidence, or gain privilege. | A steered mapping to a *different valid* canonical is possible, but that canonical's evidence must still be cryptographically presented. Integrity holds. |
| **Hallucinated / fabricated id** (LLM09 Overreliance) | Any id not in the registry is rejected by Gate 2 *by construction* (engine membership re-check). | None to integrity; only availability of the paraphrase path. |
| **Sensitive-information disclosure** (LLM06) | G3 is *structural*: the `QuestionResolver` interface has no field for subjectId, credentials, evidence, or prior answers — no code path can carry PII to the model. | None (structural, not a policy that could be misconfigured). |
| **Excessive agency** (LLM08) | The model has no tools and takes no actions; its sole output is a JSON proposal the engine disposes of. | None. |
| **Insecure output handling** (LLM02) | The proposal is parsed as strict JSON and membership-checked; it is never executed, `eval`-ed, or rendered as markup. | None. |
| **System-prompt leakage** (LLM07) | The prompt contains only the *public* canonical catalogue (ids + one-line descriptions). | None — nothing secret to leak. |
| **Denial-of-wallet / model DoS** (LLM04) | Only Tier-1 *misses* ever call the model (Tier-1 hits are free); each call is bounded by a 3 s timeout and fails closed with no blocking retry. | A looped unresolvable question still costs one bounded model call per attempt. Budget cap, rate window, circuit breaker, and a negative cache (one call per TTL) are **designed but not implemented in this POC**. |
| **Model outage / unavailability** | G6 fail-closed: timeout/error → ABSTAIN → 422 refusal. The deterministic Tier-1 path and all evidence verification are unaffected. | The paraphrase convenience path is unavailable during an outage; the core decision path is not. |
| **Training-data poisoning / model integrity** (LLM03) | The design *assumes* an untrusted model; no decision depends on the model being correct or honest. | Out of scope by assumption — see residual (b). |

**Residual risks (stated plainly).**

- **(a) Alias promotion — the one place an AI output could persist.** Today every
  LLM-validated mapping is *per-request*: nothing is written back to the registry,
  so an AI output has no persistence and no cross-user effect. *If* a future
  optimization promoted validated mappings into persistent `questions.yaml`
  aliases, that would be the single point where an AI output gains persistence and
  cross-user reach — always to a *valid* canonical (so integrity still holds), but
  a misleading alias becomes possible. Production mitigation: human-approved
  promotion + audit trail. (Not implemented in this POC — noted here for honesty.)
- **(b) Upstream model integrity.** This design makes no claim about the model's
  training-data integrity or provenance. It assumes an untrusted model — which is
  precisely why none of the above depends on the model being good.


## 5. Evidence, predicates, policies, reverse inference and privacy ranking

The knowledge base is config-driven YAML, so it can be extended without code
changes.

### 5.1 The three configuration layers

- **`questions.yaml`** — canonical ids and their natural-language aliases
  (Tier 1).
- **`predicates.yaml`** — each predicate's acceptable evidence options, each with
  an assurance level, plus an optional `prefer` field naming the privacy-minimal
  option. Example: `age_over_18` accepts `AGE_ATTESTATION` (HIGH) *or*
  `GOVERNMENT_IDENTITY` (HIGH, a DOB-bearing credential), but `prefer:
  AGE_ATTESTATION` ranks the minimal option first.
- **`policies.yaml`** — each policy's required predicates in significant order.
  Example: `BANK_ACCOUNT_OPENING_V1` (version 1) requires
  `identity_verified`, `age_over_18`, `eu_resident`, and `consent_valid`.

**Order is significant everywhere** — evidence options within a predicate, and
required predicates within a policy — because it drives deterministic output
ordering.

### 5.2 Reverse inference and privacy ranking

`inference::privacy_ordered` emits a predicate's evidence options **privacy-first**:
the `prefer` type is listed first, then the remaining options in config order,
deduplicated. `inference::build_plan` walks the required predicates for a
canonical id and includes **only those not yet established** by the subject's
current evidence — already-satisfied predicates request nothing, which is data
minimization in the plan itself. Each included predicate contributes its
privacy-preferred evidence type to the request's `requiredEvidence` list.

`inference::established_from_evidence` computes which predicates are already
`TRUE` given the subject's known evidence states (a predicate is established when
any of its evidence options is `AVAILABLE`). This is what powers Scenario 2's
flow: an initial evaluation returns `UNKNOWN` with an
`evidenceRequestPlan[RESIDENCE_CREDENTIAL]`; after the residence credential is
granted, re-evaluation returns `ALLOW`.

### 5.3 Deterministic policy evaluation

`policy::evaluate` returns:

- **`DENY`** only when evidence affirmatively establishes falsity, or is
  `INVALID`/`REVOKED`.
- **`UNKNOWN`** when required evidence is `MISSING`/`EXPIRED`/unresolved — the
  first-class outcome, accompanied by an `evidenceRequestPlan`.
- **`ALLOW`** when all required predicates are satisfied.

The `DecisionReport` carries `verifiedClaims` (derived booleans only, never raw
values), `satisfiedPredicates`, `missingPredicates`, `evidenceUsed`,
`evidenceIgnored`, `reasons`, and the injected `evaluationTime`.

---

## 6. Real SD-JWT VC pipeline

This is genuine cryptography end to end: RustCrypto `p256`/ES256 in the engine,
and `java.security` ES256 in the wallet. No fake algorithms.

### 6.1 Issuance (`issuer/mod.rs`, `crypto.rs`)

The demo PID issuer holds a static ES256 keypair generated at startup and
publishes its public key as a JWKS at `GET /engine/issuer/jwks`. **That published
key is the trust-list stand-in for this demo** — a real deployment would resolve
issuer keys from an eIDAS/EUDI trust list.

`Issuer.issue` produces an IETF SD-JWT VC:

- For **each** claim (`given_name`, `family_name`, `birth_date`,
  `resident_country`) it builds a disclosure `[salt, name, value]`, base64url-
  encodes it, and takes its SHA-256 as an `_sd` digest. Every claim is thus
  **individually selectively disclosable** behind its own salted digest. Salts
  are random — allowed non-determinism that never enters a decision body.
- The `_sd` digest array is **sorted** so the issuer JWT does not leak claim
  order.
- The payload carries `iss`, `iat`, `exp` (24h TTL), `vct`
  (`urn:eu.europa.ec.eudi:pid:1`), `_sd_alg: sha-256`, `_sd`, and
  **`cnf.jwk`** holding the holder's public JWK for key binding.
- The JWT is signed ES256 (`crypto::jws_sign`); the returned combined form is
  `<sd-jwt>~<disclosure>*~`.

`POST /engine/issuer/issue {subjectId, holderJwk}` returns the SD-JWT and its
disclosures.

### 6.2 Wallet-side selective disclosure + Key-Binding JWT (`MockWallet.java`, `JwtCrypto.java`)

The wallet is a *mock* only in the sense that it is **not a certified wallet** (no
secure element, no wallet attestation) — the cryptography it performs is real. On
issuance it generates its own ES256 holder keypair, requests the PID SD-JWT bound
to that key via `cnf`, and stores the SD-JWT plus all disclosures.

On presentation it **minimizes disclosure locally**:

- `claimsToDisclose` maps required evidence to the minimum claims — an
  `AGE_ATTESTATION` request maps to `birth_date` **only** (never name or
  country). Withheld claims are logged.
- It assembles `presented = <issuer-jwt> ~ <selected-disclosure>* ~`, computes
  `sd_hash = SHA-256-base64url(presented)`, and signs a **Key-Binding JWT** with
  the holder private key: header `{alg: ES256, typ: kb+jwt}`, payload
  `{iat, aud, nonce, sd_hash}`. The final VP is `presented + kb-jwt`.

`JwtCrypto.signEs256` produces a real compact JWS with a JOSE raw `R||S`
signature (DER→raw conversion), verifiable by the engine against the credential's
`cnf` key.

### 6.3 Verifier checks a–f, named results and failure codes (`evidence/mod.rs`)

`SdJwtCredentialVerifier` (the live wiring; `MockCredentialVerifier` is retained
for tests) splits the VP on `~` into `<issuer-jwt> ~ <disclosure>* ~ <kb-jwt>` and
runs these checks, each recorded as a `CheckResult` in the response and audit:

| # | Check (named result) | What it verifies | Failure code on error |
|---|---|---|---|
| (pre) | *presence* | a non-empty `sdJwtVp` exists | `MISSING_SD_JWT_VP` |
| (pre) | *structure* | at least issuer JWT + KB-JWT, KB-JWT non-empty | `MALFORMED_VP` |
| a | `ISSUER_SIGNATURE` | issuer JWT signature valid against the pinned issuer key | `INVALID_SIGNATURE` |
| a′ | *(issuer identity)* | `iss` matches the trust anchor | `UNTRUSTED_ISSUER` |
| b | `DISCLOSURE_DIGESTS` | every disclosure hashes to a digest present in `_sd` | `UNKNOWN_DISCLOSURE` |
| c | `CREDENTIAL_VALIDITY` | `exp` not before the injected clock `now` | `EXPIRED_CREDENTIAL` |
| d | `KEY_BINDING` | KB-JWT signed by `cnf` key; `aud` and `sd_hash` match | `KEY_BINDING_MISMATCH` |
| e | `NONCE_BINDING` | KB-JWT `nonce` equals the request nonce | `NONCE_MISMATCH` |
| f | `CLAIM_DERIVATION` | derive boolean claims; **raw values discarded** | (n/a — always after checks) |

Any failed check short-circuits to `DENY` with the named `failedCheck` and the
`verificationChecks[]` recorded so far, plus `disclosureCount`. When all checks
pass but a required disclosure is simply absent, the result is `UNKNOWN` (not
`DENY`) — a missing fact is not a falsified fact.

### 6.4 Derived-claim-only output (birth_date never leaves the verifier)

`derive_evidence` maps each required evidence type to the SD-JWT claim that
establishes it (`AGE_ATTESTATION → birth_date`, `GOVERNMENT_IDENTITY →
family_name`, `RESIDENCE_CREDENTIAL → resident_country`) and derives an
**evidence state**, never a raw value:

- `age_over_18` is computed by `is_over_18(birth_date, now)` against the injected
  clock. Only the boolean reaches the policy; the `birth_date` value appears in
  **no** decision response and **no** audit record.
- An affirmatively false derived claim (under-18, or a non-EU residence value)
  yields `INVALID` → `DENY`. A missing disclosure yields `MISSING` → `UNKNOWN`.

The module header states the invariant plainly: *"Raw attribute VALUES never
leave this module."*

### 6.5 The `CredentialVerifier` trait seam

Both verifiers implement one trait:

```rust
pub trait CredentialVerifier: Send + Sync {
    fn verify(&self, presentation: &Presentation, challenge: &Challenge,
              now: OffsetDateTime) -> VerificationResult;
}
```

The injected `now` is the evaluation clock (determinism). The real
`SdJwtCredentialVerifier` pins the issuer public key at construction (the
trust-list stand-in). Swapping in a future OpenID4VP + trust-list verifier is a
matter of providing another implementation of this trait; callers are unchanged.

---

## 7. Replay protection (`replay/mod.rs`)

Replay protection is real, not mocked.

- **Cryptographic single-use nonce.** Each evidence request creates a
  `Challenge` with a 256-bit nonce from `OsRng` (hex-encoded), bound to
  `requestId`, `audience`, `subjectId`, resolved `canonical`/`kind`, and
  `requiredEvidence`.
- **TTL.** Nonces expire after `NONCE_TTL_SECONDS = 300` (5 minutes); expiry is
  checked against the injected clock.
- **Atomic single-use consumption.** `ReplayStore::validate_and_consume` holds
  the store lock while it validates the nonce/audience/request-id, checks
  expiry and prior consumption, and — only on success — sets `consumed = true`
  before releasing the lock. A replayed VP therefore returns
  `NONCE_ALREADY_CONSUMED`.
- **`REPLAY_DETECTED` is a decision.** The `NONCE_ALREADY_CONSUMED` replay result
  surfaces to the caller as a `REPLAY_DETECTED` decision; other nonce failures
  map to distinct codes (`UNKNOWN_NONCE`/`EXPIRED_NONCE`/`WRONG_AUDIENCE`/
  `WRONG_REQUEST_ID`).
- **`ReplayStore` seam.** `InMemoryReplayStore` is the current backing; the trait
  is Redis-swappable for multi-instance nonce consumption without touching
  callers.

---

## 8. Determinism guarantees and how they are tested

The contract is **same input + same state ⇒ byte-identical decision body.**

- **Injected clock.** Evaluation accepts an `evaluationTime` (RFC3339) and
  defaults to now. All time-sensitive logic — credential expiry, nonce TTL, the
  `is_over_18` age computation — reads this clock, so a frozen clock produces a
  fixed result. `verify` takes `now` as a parameter for the same reason.
- **Sorted JSON.** Output maps use `BTreeMap` (sorted keys); the crate uses
  `serde_json` with `preserve_order`. No `HashMap` iteration appears on output
  paths.
- **Config-ordered arrays.** Evidence options follow `prefer`-then-config order;
  policy predicates follow config order; issuer `_sd` digests are explicitly
  sorted. Ordering is never left to hash iteration.
- **Randomness is confined and excluded from decisions.** The only
  non-determinism is cryptographic key generation, disclosure salts, and nonce
  generation (`OsRng`). None of these values enter a decision body. `crypto.rs`
  notes it directly at `generate_key`: *"randomness is confined here — it never
  enters a decision body."*
- **`auditId` is added at the API layer.** The per-call fresh UUID lives outside
  the engine's decision body, so it never breaks the determinism contract.

**How it is tested.** A dedicated test (`determinism_byte_identical`) evaluates
the same question + same evidence + same frozen clock twice and asserts the JSON
is byte-identical. The property can also be checked live against the engine:

```bash
BODY='{"canonical":"age_over_18","evidence":[{"type":"AGE_ATTESTATION","state":"AVAILABLE"}],"evaluationTime":"2020-01-01T00:00:00Z"}'
A=$(curl -s localhost:8081/engine/evaluate -H 'content-type: application/json' -d "$BODY")
B=$(curl -s localhost:8081/engine/evaluate -H 'content-type: application/json' -d "$BODY")
diff <(echo "$A" | jq 'del(.auditId)') <(echo "$B" | jq 'del(.auditId)') && echo IDENTICAL
```

(`auditId` is stripped because it is the one intentional per-call difference.)

---

## 9. Security model and threat considerations

- **Untrusted LLM.** Treated as an adversary at the edge. It cannot reach PII
  (G3, structural), cannot bypass closed-set membership (G2, engine-side
  re-validation), cannot influence decisions (G4, zero LLM calls in the core),
  and cannot fail open (G6). Worst case, a compromised model influences only
  *which permitted canonical question* is asked — and the engine still rejects
  anything outside the registry.
- **Tampering.** A modified or fabricated disclosure fails
  `DISCLOSURE_DIGESTS` (`UNKNOWN_DISCLOSURE`); a tampered issuer JWT fails
  `ISSUER_SIGNATURE` (`INVALID_SIGNATURE`); a swapped holder key or altered
  presentation fails `KEY_BINDING` (`KEY_BINDING_MISMATCH`, via the `sd_hash`
  binding over the presented combination).
- **Replay.** Single-use nonces with atomic consumption and TTL (§7);
  resubmission returns `REPLAY_DETECTED`.
- **Expiry / staleness.** Credential `exp` is enforced against the injected
  clock (`EXPIRED_CREDENTIAL`); missing/expired evidence yields first-class
  `UNKNOWN` rather than a silent wrong answer.
- **Key binding.** The KB-JWT proves holder possession of the `cnf` private key
  and binds the presentation to `aud`, `nonce`, and the exact disclosed set
  (`sd_hash`), preventing presentation splicing and mis-audience replay.
- **PII-free audit.** Audit records store evidence **types and states**,
  predicate states, `resolvedBy`, the LLM proposal JSON (non-PII), verification
  checks, and `disclosureCount` — but never raw attribute values.
- **Trust-list stand-in.** The single pinned issuer key substitutes for a real
  trust list. This is a *demo simplification, stated as such* — see §10.

---

## 10. REAL vs. MOCKED vs. FUTURE

This section is authoritative; earlier sections are scoped by it.

### REAL (implemented and exercised)

- **Real SD-JWT VC issuance** with per-claim salted `_sd` digests, `cnf` key
  binding, and `vct` (ES256, RustCrypto).
- **Selective disclosure** and a real **Key-Binding JWT** (`nonce`, `aud`,
  `sd_hash`) signed by the holder key (ES256, `java.security`).
- **Real verification**: the six named checks with distinct failure codes, and
  derived-claim-only output (raw `birth_date` never leaves the verifier).
- **Two-tier question resolution** with a deterministic registry + a
  closed-world, guardrailed, PII-free, fail-closed LLM proposer.
- **Reverse evidence inference** (question → predicates → privacy-ranked
  evidence) with plan minimization.
- **Policy evaluation with first-class `UNKNOWN`.**
- **Cryptographic single-use nonce / replay protection** with TTL.
- **Privacy-minimized responses** (derived booleans, never raw attributes).
- **PII-free audit / provenance**, including `resolvedBy` and the recorded LLM
  proposal.
- **The Rust engine ↔ Java orchestration boundary** over explicit HTTP DTOs.

> When running with `StubLlmResolver` (no API key), the AI path is honestly a
> *stubbed model with the identical guardrail pipeline* — the guardrails,
> validation, and audit trail are real either way; only the model call is
> substituted.

### MOCKED (deliberate, honest simplifications)

- The **wallet is not a certified wallet** — no secure element, no wallet
  attestation. Its cryptography is real; its assurance posture is not.
- The **single demo issuer key stands in for a trust list.** There is no
  issuer-key resolution, rotation, or algorithm agility.
- `MockCredentialVerifier` performs **no cryptography** and is retained for
  tests only; it does not invent a fake algorithm — it trusts presented states
  behind the same real trait seam. The live path is the real
  `SdJwtCredentialVerifier`.

### FUTURE (designed-for integration, not implemented)

- **OpenID4VP / OpenID4VCI** presentation and issuance transport.
- **mdoc (ISO/IEC 18013-5)** proof verification alongside SD-JWT.
- **Real trust lists** and issuer-key resolution; **wallet attestation.**
- **Redis-backed `ReplayStore`** for multi-instance nonce consumption (same
  trait).

### Never claimed

This POC does **not** claim, and must not be represented as, any of the
following:

- a complete EUDI implementation;
- eIDAS certification;
- production EU-regulatory compliance;
- OpenID4VP / OpenID4VCI interoperability;
- a certified wallet (secure element / wallet attestation);
- a production trust-list or issuer-key-management system.

---

## 11. Future work and EUDI integration path

The seams are deliberate, so integration is additive rather than a rewrite:

- **Verification.** Replace `SdJwtCredentialVerifier`'s pinned key and add an
  OpenID4VP presentation exchange plus SD-JWT / mdoc proof verification against
  real **issuer trust lists** and **wallet attestation** — all behind the
  existing `CredentialVerifier` trait, so callers are unchanged.
- **Issuance.** Move from the demo PID issuer to OpenID4VCI-based issuance and
  trust-list-resolved issuer keys with rotation and algorithm agility.
- **Replay.** Swap `InMemoryReplayStore` for a Redis-backed `ReplayStore`
  (same trait) for horizontal scaling.
- **Knowledge base.** Registry, predicates, and policies are already
  config-driven YAML; extend the supported questions and policies without code
  changes.
- **AI edge.** The guardrail pipeline (G1–G6) is model-agnostic; new proposers
  plug in behind the `QuestionResolver` interface with the same structural PII
  isolation.

The consistent thesis: **let AI improve usability at the edge, and keep the
evidence → verification → decision chain deterministic, minimal, and
auditable.** The LLM proposes; the engine disposes.

---

*Proof-of-concept / demo. See §10 for the exact REAL / MOCKED / FUTURE boundary
and the explicit "Never claimed" list.*
