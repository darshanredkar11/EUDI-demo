# Identity Evidence Inference Demo

A working proof-of-concept that answers a **business question** about a person by
inferring *what evidence is required to answer it*, requesting only the minimum,
verifying it, and returning a **deterministic** decision — `ALLOW` / `DENY` /
`UNKNOWN`.

The differentiated capability is **not** credential verification. It is:

> **QUESTION → WHAT MUST BE TRUE? → WHAT EVIDENCE PROVES IT? → WHAT IS THE
> MINIMUM INFORMATION NEEDED? → REQUEST → VERIFY → DETERMINISTIC DECISION**

---

## 1. Problem

A relying party asks *"Can this user open a bank account?"* or *"Is this user
over 18?"*. Most systems receive a Verifiable Credential and check a signature.
That skips the interesting part: **given a human question, decide what must be
proven, and disclose as little as possible to prove it.** Stale or missing
information must never silently produce a wrong answer — `UNKNOWN` is a
first-class decision, not an error.

## 2. Architecture

```
Business Question (natural language)
        │
        ▼
┌──────────────────────── Java Spring Boot :8080 — thin API + orchestration ────────────────────────┐
│  VerificationController                                                                              │
│      → VerificationService (orchestration only; NO business/policy logic)                           │
│            ├─ QuestionResolver  (Tier 2: StubLlmResolver | AnthropicLlmResolver)  ← untrusted       │
│            ├─ EngineClient (RestClient)                                                              │
│            └─ MockWallet                                                                             │
└─────────────────────────────────────────────┬───────────────────────────────────────────────────────┘
                                               │ HTTP JSON (explicit DTOs both sides)
                                               ▼
┌──────────────────── Rust axum engine :8081 — deterministic decision core ────────────────────┐
│  resolution/  (Tier 1 registry + proposal validation)                                          │
│  inference/   (REVERSE inference: canonical → predicates → evidence, privacy-ranked)            │
│  policy/      (evaluation → ALLOW | DENY | UNKNOWN, first-class UNKNOWN)                         │
│  evidence/    (RAW_ATTRIBUTE vs DERIVED_CLAIM; CredentialVerifier seam — MockCredentialVerifier)│
│  replay/      (real nonce/replay protection; ReplayStore seam — InMemoryReplayStore)            │
│  audit/       (provenance: types+states, never PII)                                             │
│  config/      questions.yaml · predicates.yaml · policies.yaml                                  │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

**Boundary (non-negotiable):** all identity/policy business logic lives in Rust.
Java only translates the external API to the engine contract and orchestrates the
two-tier question resolution. There is **no AI dependency in the engine** — the
trust boundary is visible in the architecture itself.

## 3. The reverse-inference concept (the GIE connection)

This system is the **reverse** of a Graph-based Inference Engine (GIE):

| | direction |
|---|---|
| **GIE** | knowledge → inference → answer / recommendation |
| **This system** | question → required predicates → required evidence → verification → answer |

Shared philosophy inherited from GIE: deterministic inference; explicit facts;
stale/missing information never silently yields a wrong answer; `UNKNOWN` is
legitimate; full provenance; **same input + same state = same result**. The
reverse direction is why the modules are named `resolution/` and `inference/`.

## 4. Question resolution — two-tier, and why it stays deterministic

Resolution maps a human question to a **canonical predicate or policy**. The
decision core is 100% deterministic — **the LLM PROPOSES, the engine DISPOSES.**

### Tier 1 — deterministic registry (Rust, `config/questions.yaml`)

Algorithm (exactly this):
1. Normalize input: lowercase → trim → collapse internal whitespace → strip
   terminal punctuation (`?.!`).
2. Exact-match the normalized string against the normalized alias table.
3. If the input already **is** a canonical id (e.g. `age_over_18` or
   `BANK_ACCOUNT_OPENING_V1`), accept it directly.
4. No match → fall through to Tier 2.

### Tier 2 — guardrailed LLM proposer (Java)

**Trust boundary (verbatim):**

> The LLM is an UNTRUSTED PROPOSER outside the decision core. It maps a
> paraphrased question to ONE member of a closed set of canonical
> predicates/policies, or ABSTAIN. Every proposal is validated against the
> deterministic registry before use. The LLM never sees credentials, VCs, VPs,
> subject attributes, or any PII. It never participates in evidence evaluation,
> verification, replay protection, or decisions.

**AI attack surface.** *We never try to make the model behave; we make misbehavior
harmless.* Every AI attack collapses to one of two outcomes: a **refusal** (fail
closed), or an outcome the attacker **could have had by asking plainly** — because
they only ever control their own question. Integrity is never at stake; only the
availability of the paraphrase convenience path. Injection and hallucination die at
Gate 2 (closed-set membership re-check); PII can't reach the model (G3 is
structural); the model has no tools and its output is strict JSON, never executed.
Full attack-class table (with OWASP LLM Top-10 ids and residual risks) in
[WHITEPAPER.md](WHITEPAPER.md#4a-ai-threat-model).

**Guardrails (all enforced, each demonstrable):**

- **G1 Closed-world output** — the prompt enumerates the canonical set with
  one-line descriptions and demands strict JSON:
  `{ "canonical": "<id>"|null, "confidence": "HIGH|MEDIUM|LOW", "reason": "<short>" }`.
- **G2 Engine-side validation** — a proposal is accepted only if `canonical`
  exactly matches a registry entry **and** `confidence == HIGH`. Anything else
  (unknown id, MEDIUM/LOW, null, malformed JSON, timeout) → `UNRESOLVED_QUESTION`
  (HTTP 422). Confidence is gated in Java; **membership is re-validated by the
  engine** (`POST /engine/resolve` with `proposedCanonical`). The engine never
  trusts the LLM's string.
- **G3 Data minimization for AI** — the resolver receives ONLY the raw question
  + the catalogue (ids + descriptions). Enforced *structurally*: the
  `QuestionResolver` interface has no field for subjectId/credentials/evidence.
- **G4 Deterministic core untouched** — resolution feeds the SAME engine path as
  Tier 1. Temperature 0. The decision/evaluation path contains zero LLM calls.
- **G5 Auditability** — an LLM-assisted resolution records `resolvedBy:
  LLM_VALIDATED`, the model id, the proposal JSON, and the validation outcome.
  Every response also carries `resolvedBy`.
- **G6 Fail closed** — LLM unavailable/timeout/error → `UNRESOLVED_QUESTION`,
  never a guess, never a blocking retry loop. Timeout 3s.

**Why this is the correct EUDI+AI architecture:** AI improves usability *at the
edge* (understanding a paraphrased question) while the ARF-style trust chain
(evidence → verification → decision) stays deterministic and auditable. Data
minimization is applied to the AI component itself — the model never receives
credential data. **Guessing is impossible by construction:** the only ways
forward are a validated member of the closed set, or a refusal.

**Live/stub toggle (one line):** set `ANTHROPIC_API_KEY` to use the real
`AnthropicLlmResolver` (model `claude-sonnet-4-6`, temperature 0); unset it and
the default `StubLlmResolver` runs — *stubbed model, identical guardrail
pipeline*. The demo works identically either way.

## 5. Running it

Prereqs: Docker + `jq`. (For local non-Docker runs: Rust stable, JDK 21.)

```bash
cd EUDI
docker compose up --build         # starts engine (:8081) and api (:8080)
./demo/run_demo.sh                # runs all SIX scenarios, asserts outcomes
```

### No Docker — `make run`

No Docker daemon needed; the `Makefile` builds and runs both services as
plain local processes, tracked by PID file + health check (same ports, same
env vars as `docker-compose.yml`):

```bash
cd EUDI
make run      # builds (first time) and starts engine + api, waits for health
make ui       # opens the demo console at localhost:8080
make demo     # runs the 6-scenario scripted demo against the running stack
make status   # what's running, and whether it's healthy
make logs     # tail both service logs
make stop     # stop both
```

Requires JDK 21 specifically — `make` finds it automatically (macOS via
`java_home`, Linux/WSL2 via `JAVA_HOME` or a search of `/usr/lib/jvm`);
Byte Buddy/Mockito reject JDK 24, see §11. `ANTHROPIC_API_KEY=sk-...
make run` enables the real LLM proposer; unset it and `StubLlmResolver` runs
with an identical guardrail pipeline.

**Windows:** plain `cmd`/PowerShell has no `make` and no POSIX shell for
these recipes, so the Makefile isn't usable as-is there. Two options that do
work, no changes needed:
- **Docker Desktop** — `docker compose up --build` runs unmodified on Windows.
- **WSL2** — install an Ubuntu distro, `apt install openjdk-21-jdk` +
  `cargo`/Rust inside it, then run this same `Makefile` from the WSL shell.
  `make ui` will pop the Windows browser automatically (falls back through
  `xdg-open` to `powershell.exe Start-Process` for WSL interop).

### Demo UI — open <http://localhost:8080/>

A one-page, click-through **demo console** for non-technical evaluators (no
Swagger, no curl). Three columns tell the story — **Relying Party → Wallet →
Engine** — and each scenario button drives the real `/v1` API, animating the
columns as responses arrive: selective disclosure (birth_date ✓ / name+country
✗ withheld), the six verification checks flipping to ✓, and the decision badge
(ALLOW / UNKNOWN / DENY / REPLAY_DETECTED / REFUSED). Every exchange is viewable
as raw JSON. It is a pure client of the existing API — no backend endpoint was
added or changed for it, and it shows an explicit "engine unreachable" state
rather than ever faking output. Vanilla HTML/CSS/JS, served by Spring Boot from
`java-api/src/main/resources/static/` (no build step).

**Swagger UI (run every scenario from the browser):**
- Java API: <http://localhost:8080/swagger-ui.html> (spec `/v3/api-docs`)
- Rust engine: <http://localhost:8081/swagger-ui> (spec `/api-docs/openapi.json`)

Every endpoint ships realistic `Try it out` examples grouped by flow
(`1-Question Resolution`, `2-Wallet`, `3-Presentation & Verification`, `4-Audit`,
`5-Issuer`).

The demo needs **no** `ANTHROPIC_API_KEY` (StubLlmResolver path). To use the
real model: `ANTHROPIC_API_KEY=sk-... docker compose up --build`.

### Run without Docker

```bash
# terminal 1 — engine
cd rust-engine && cargo run --bin engine        # :8081

# terminal 2 — api  (JDK 21 required; see Design decisions)
cd java-api && ./mvnw spring-boot:run            # :8080

# terminal 3
./demo/run_demo.sh
```

### Hosting it (a live URL, not a zip)

For letting someone try this without setting up JDK 21/Rust locally:

**One-click (Render):** this repo ships a [`render.yaml`](render.yaml)
blueprint — push to GitHub, then Render → New → Blueprint → point at the
repo. It deploys `eudi-engine` as a **private** service (no public URL —
matches the project's own trust-boundary claim: the deterministic core
isn't internet-reachable) and `eudi-api` as the public one. Free plan has no
card requirement; the tradeoff is a ~30-60s cold start after ~15 min idle —
either warm the URL a couple of minutes before a live demo, or bump `plan:
free` → `plan: starter` (~$7/mo/service) in the blueprint for always-on.
Set `ANTHROPIC_API_KEY` in the Render dashboard only if you want the real
LLM proposer live; unset it and `StubLlmResolver` runs (no cost, no key on
a public box).

**Small VPS (more control, always warm):** Hetzner CX22 (~€4.2/mo) or a
DigitalOcean $6/mo droplet, Docker installed, this repo's
`docker-compose.yml` unmodified, Caddy in front for automatic HTTPS,
only port 8080 exposed publicly (engine stays on the internal compose
network — not internet-reachable, same story as the Render private service).

Both paths use the exact same containers as local Docker — nothing is
reconfigured for hosting beyond the two knobs above (dynamic `PORT` binding,
already wired for both services).

## 6. API examples (copy-paste curl)

```bash
# Scenario 1 — age check (privacy-minimized)
curl -s localhost:8080/v1/verification/questions -H 'content-type: application/json' \
  -d '{"subjectId":"user-123","question":"Is this user over 18?"}' | jq
# → { requestId, canonical:"age_over_18", kind:"predicate",
#     requiredEvidence:["AGE_ATTESTATION"], nonce, expiresAt, resolvedBy:"REGISTRY" }

# present (MOCK wallet) then verify → ALLOW, verifiedClaims.age_over_18=true (no DOB)
curl -s localhost:8080/v1/verification/presentations -H 'content-type: application/json' \
  -d '{"requestId":"<id>","presentation":{ ... }}' | jq

# Scenario 2 — policy, UNKNOWN with an evidence request plan
curl -s localhost:8080/v1/verification/decisions -H 'content-type: application/json' \
  -d '{"subjectId":"user-123","policy":"Can this user open a bank account?"}' | jq

# Scenario 5 — out of scope → 422
curl -s localhost:8080/v1/verification/questions -H 'content-type: application/json' \
  -d '{"subjectId":"user-123","question":"Can this user pilot a plane?"}' | jq
# → 422 { error:"UNRESOLVED_QUESTION", supportedQuestions:[...] }

# Provenance
curl -s localhost:8080/v1/audits/<auditId> | jq
```

**External API:** `POST /v1/verification/questions`, `POST
/v1/verification/presentations`, `POST /v1/verification/decisions`, `GET
/v1/audits/{id}`, plus MOCK wallet `POST /v1/wallet/present`, `POST
/v1/wallet/grant`.
Uniform error body: `{ "error": CODE, "message", "details" }` — codes:
`UNRESOLVED_QUESTION`, `UNKNOWN_REQUEST`, `EXPIRED_NONCE`, `REPLAY_DETECTED`,
`ENGINE_UNAVAILABLE`, `VALIDATION_ERROR`.

**Engine contract (Spring-independent):** `POST /engine/resolve`,
`POST /engine/validate-canonical`, `POST /engine/plan`, `POST /engine/evaluate`,
`POST /engine/presentations`, `GET /engine/audits/{id}`, `GET /health`.

## 7. Determinism guarantees

- Evaluation uses an **injected clock**: the engine accepts `evaluationTime`
  (RFC3339) and defaults to now. Frozen clock + same input = identical output.
- All JSON maps use sorted keys (`BTreeMap`); arrays follow config order then
  lexicographic. No `HashMap` iteration on output paths.
- No randomness in evaluation — randomness is confined to nonce generation
  (`OsRng`).
- Enforced by a test (`determinism_byte_identical`): same question + same
  evidence + same frozen clock evaluated twice returns **byte-identical** JSON.

Check it directly against the engine:

```bash
BODY='{"canonical":"age_over_18","evidence":[{"type":"AGE_ATTESTATION","state":"AVAILABLE"}],"evaluationTime":"2020-01-01T00:00:00Z"}'
A=$(curl -s localhost:8081/engine/evaluate -H 'content-type: application/json' -d "$BODY")
B=$(curl -s localhost:8081/engine/evaluate -H 'content-type: application/json' -d "$BODY")
# auditId is a fresh UUID per call; strip it, the decision body is identical:
diff <(echo "$A" | jq 'del(.auditId)') <(echo "$B" | jq 'del(.auditId)') && echo IDENTICAL
```

## 8. Tests

```bash
cd rust-engine && cargo test          # units + determinism + HTTP + SD-JWT crypto
cd java-api    && ./mvnw test          # resolver, guardrails, engine contract, wallet minimisation (JDK 21)
```

`sdjwt_tests.rs` covers the real SD-JWT path: happy-path minimal disclosure,
signature tamper, disclosure tamper, wrong nonce, expired credential, wrong
key-binding key, wrong audience, and byte-identical determinism. `MockWalletTest`
asserts an age check discloses `birth_date` only.

## 9. REAL vs MOCKED vs FUTURE

**REAL**
- Two-tier question resolution: deterministic registry + guardrailed LLM
  proposer with closed-world validation, fail-closed, PII-free prompts.
- Reverse evidence inference (question → predicates → privacy-ranked evidence).
- Policy evaluation with first-class `UNKNOWN`.
- **SD-JWT VC issuance** — demo PID issuer with a real ES256 (P-256) key,
  salted `_sd` digests, `cnf` holder key binding, `vct`, JWKS endpoint.
- **Selective disclosure** — the wallet discloses only the claims the request
  needs (age check → `birth_date` only; name/country withheld).
- **Key binding** — holder-signed KB-JWT (nonce, aud, `sd_hash`), verified
  against the credential's `cnf` key.
- **Cryptographic verification** — `SdJwtCredentialVerifier` checks issuer
  signature, disclosure digests, validity (injected clock), key binding and
  nonce; each is a named, audited check. `age_over_18` is derived from
  `birth_date` against the clock and only the boolean crosses the boundary.
- Privacy-minimized responses (derived claims, never raw attribute values).
- Nonce / replay protection (cryptographic single-use nonce, TTL) →
  `REPLAY_DETECTED`; tamper → `DENY` with the failing check named.
- Audit / provenance: `resolvedBy`, LLM proposal record, verification checks,
  disclosure count — never PII.
- Rust engine ↔ Java orchestration boundary; Swagger UI on both services.

> When running with `StubLlmResolver` (no API key), the AI path is honestly a
> *stubbed model with the identical guardrail pipeline* — the guardrails,
> validation, and audit trail are real either way; only the model call is
> substituted.

**MOCKED**
- The wallet is **not a certified wallet** (no secure element, no wallet
  attestation) — but the cryptography it performs (holder keygen, KB-JWT
  signing) is real.
- The single demo **issuer key stands in for a trust list** (published at
  `/engine/issuer/jwks`); there is no real eIDAS/EUDI trust-list resolution.
- `MockCredentialVerifier` (state-trusting) is retained for tests only; the live
  wiring is `SdJwtCredentialVerifier`.
- The policy `decisions` path (Scenario 2) still evaluates config-driven
  abstract evidence held by the wallet; the SD-JWT crypto path drives the
  presentation/age flow (Scenarios 1, 4, 6).

**DESIGNED FOR FUTURE INTEGRATION**
- OpenID4VP / OpenID4VCI transport, SD-JWT/**mdoc** interop, real trust lists,
  wallet attestation, algorithm agility and key rotation.

**Never claimed:** complete EUDI implementation, eIDAS certification, production
EU-regulatory compliance, OpenID4VP interoperability, or mdoc support (none of
these are implemented).

## 10. Future EUDI integration path

- SD-JWT VC issuance + verification is now real behind the `CredentialVerifier`
  seam. Next: wrap it in **OpenID4VP** presentation exchange and **OpenID4VCI**
  issuance transport, add **mdoc** alongside SD-JWT, and resolve issuer keys
  from a real **trust list** + **wallet attestation** (today the demo issuer key
  at `/engine/issuer/jwks` stands in for the trust list). Callers are unchanged
  (trait seam).
- Swap `InMemoryReplayStore` for a Redis-backed `ReplayStore` (same trait) for
  multi-instance nonce consumption.
- Registry, predicates and policies are already config-driven YAML — extend the
  knowledge base without code changes.

## 11. Design decisions log

- **Two containers only** (engine + api); no K8s/Kafka/Redis/DB per scope guard.
- **HTTP, not FFI**, between Java and Rust — a clean, demonstrable service seam.
- **Registry lives in Rust** (it is knowledge, not orchestration); Java passes
  the raw question through and orchestrates Tier 2 on a miss.
- **LLM confidence gate in Java, membership re-validation in the engine** — G2 is
  the *conjunction* of both; the engine alone guarantees closed-set membership.
- **`REPLAY_DETECTED` is a decision**, surfaced from the `NONCE_ALREADY_CONSUMED`
  replay result; other nonce failures map to distinct HTTP error codes.
- **MockWallet exposes `/grant`** so Scenario 2 can acquire the residence
  credential and re-evaluate to `ALLOW` via plain API calls (no hidden state).
- **`auditId` is added at the API layer**, keeping the decision body itself
  byte-deterministic (the UUID never enters the determinism contract).
- **JDK 21 for the Java build/tests.** Byte Buddy (Mockito) used by the tests
  does not support JDK 24; the build targets Java 21 and the Docker image uses
  `eclipse-temurin-21`. Run local `mvnw`/`mvn` with a JDK 21 `JAVA_HOME`.
- **`serde_json` `preserve_order` + `BTreeMap`** for stable, sorted JSON output.
