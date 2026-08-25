#!/usr/bin/env bash
#
# Identity Evidence Inference Demo — the five-scenario interview story.
#
# Runs against the two running services (Java API on :8080, Rust engine on :8081).
# Works with NO ANTHROPIC_API_KEY set (StubLlmResolver path). Prints request ->
# response for every step and asserts the expected outcome; exits non-zero on any
# mismatch.
#
# Usage:  ./demo/run_demo.sh        (after `docker compose up` or running both locally)
set -uo pipefail

API="${API_URL:-http://localhost:8080}"
SUBJECT="user-123"
FAILURES=0

bold() { printf "\033[1m%s\033[0m\n" "$1"; }
hr()   { printf -- "----------------------------------------------------------------------\n"; }

# check <label> <actual> <expected>
check() {
  local label="$1" actual="$2" expected="$3"
  if [[ "$actual" == "$expected" ]]; then
    printf "  \033[32mPASS\033[0m  %s = %s\n" "$label" "$actual"
  else
    printf "  \033[31mFAIL\033[0m  %s = %s (expected %s)\n" "$label" "$actual" "$expected"
    FAILURES=$((FAILURES + 1))
  fi
}

post() { # post <path> <json>
  curl -s -X POST "$API$1" -H 'Content-Type: application/json' -d "$2"
}
get() { curl -s "$API$1"; }

show() { # show <label> <json>
  echo "  $1:"
  echo "$2" | jq . | sed 's/^/    /'
}

wait_for_services() {
  bold "Waiting for services at $API ..."
  for _ in $(seq 1 60); do
    code=$(curl -s -o /dev/null -w '%{http_code}' "$API/v1/audits/none" || true)
    if [[ "$code" != "000" ]]; then echo "  services are up (probe HTTP $code)"; return 0; fi
    sleep 1
  done
  echo "  services did not become ready" >&2
  exit 1
}

wait_for_services
echo

# ======================================================================
bold "SCENARIO 1 — Privacy-minimized age check"
hr
Q1='{"subjectId":"'"$SUBJECT"'","question":"Is this user over 18?"}'
echo "  > POST /v1/verification/questions"
echo "    $Q1"
R1=$(post /v1/verification/questions "$Q1")
show "response" "$R1"
REQ_ID=$(echo "$R1" | jq -r '.requestId')
NONCE=$(echo "$R1" | jq -r '.nonce')
REQ_EV=$(echo "$R1" | jq -c '.requiredEvidence')
check "resolvedBy" "$(echo "$R1" | jq -r '.resolvedBy')" "REGISTRY"
check "requiredEvidence" "$REQ_EV" '["AGE_ATTESTATION"]'

echo "  > POST /v1/wallet/present  (MOCK wallet builds the presentation)"
WP=$(jq -n --arg s "$SUBJECT" --arg r "$REQ_ID" --arg n "$NONCE" --argjson e "$REQ_EV" \
        '{subjectId:$s,requestId:$r,nonce:$n,requiredEvidence:$e}')
PRES1=$(post /v1/wallet/present "$WP")
show "presentation" "$PRES1"

echo "  > POST /v1/verification/presentations"
PBODY1=$(jq -n --arg r "$REQ_ID" --argjson p "$PRES1" '{requestId:$r,presentation:$p}')
D1=$(post /v1/verification/presentations "$PBODY1")
show "response" "$D1"
check "decision" "$(echo "$D1" | jq -r '.decision')" "ALLOW"
check "verifiedClaims.age_over_18" "$(echo "$D1" | jq -r '.verifiedClaims.age_over_18')" "true"
check "no DOB leaked" "$(echo "$D1" | grep -ci 'date_of_birth\|dob')" "0"
echo

# ======================================================================
bold "SCENARIO 2 — UNKNOWN as a first-class decision, then ALLOW"
hr
Q2='{"subjectId":"'"$SUBJECT"'","policy":"Can this user open a bank account?"}'
echo "  > POST /v1/verification/decisions"
echo "    $Q2"
D2A=$(post /v1/verification/decisions "$Q2")
show "response" "$D2A"
check "decision" "$(echo "$D2A" | jq -r '.decision')" "UNKNOWN"
check "missingPredicates" "$(echo "$D2A" | jq -c '.missingPredicates')" '["eu_resident"]'
check "plan.requiredEvidence" "$(echo "$D2A" | jq -c '.evidenceRequestPlan.requiredEvidence')" '["RESIDENCE_CREDENTIAL"]'

echo "  > POST /v1/wallet/grant  (subject acquires the residence credential; MOCK)"
G=$(post /v1/wallet/grant '{"subjectId":"'"$SUBJECT"'","evidenceType":"RESIDENCE_CREDENTIAL"}')
show "response" "$G"

echo "  > POST /v1/verification/decisions  (re-evaluate)"
D2B=$(post /v1/verification/decisions "$Q2")
show "response" "$D2B"
check "decision" "$(echo "$D2B" | jq -r '.decision')" "ALLOW"
echo

# ======================================================================
bold "SCENARIO 3 — Replay protection"
hr
echo "  > POST /v1/verification/presentations  (re-submit the EXACT Scenario 1 presentation)"
D3=$(post /v1/verification/presentations "$PBODY1")
show "response" "$D3"
check "decision" "$(echo "$D3" | jq -r '.decision')" "REPLAY_DETECTED"
check "reason" "$(echo "$D3" | jq -r '.reason')" "NONCE_ALREADY_CONSUMED"
echo

# ======================================================================
bold "SCENARIO 4 — AI-assisted resolution WITH guardrails (the showcase)"
hr
Q4='{"subjectId":"'"$SUBJECT"'","question":"Is this customer an adult?"}'
echo "  > POST /v1/verification/questions   (paraphrase NOT in the alias table)"
echo "    $Q4"
R4=$(post /v1/verification/questions "$Q4")
show "response" "$R4"
check "canonical" "$(echo "$R4" | jq -r '.canonical')" "age_over_18"
check "resolvedBy" "$(echo "$R4" | jq -r '.resolvedBy')" "LLM_VALIDATED"
REQ_ID4=$(echo "$R4" | jq -r '.requestId')
NONCE4=$(echo "$R4" | jq -r '.nonce')
REQ_EV4=$(echo "$R4" | jq -c '.requiredEvidence')

WP4=$(jq -n --arg s "$SUBJECT" --arg r "$REQ_ID4" --arg n "$NONCE4" --argjson e "$REQ_EV4" \
        '{subjectId:$s,requestId:$r,nonce:$n,requiredEvidence:$e}')
PRES4=$(post /v1/wallet/present "$WP4")
PBODY4=$(jq -n --arg r "$REQ_ID4" --argjson p "$PRES4" '{requestId:$r,presentation:$p}')
echo "  > POST /v1/verification/presentations"
D4=$(post /v1/verification/presentations "$PBODY4")
show "response" "$D4"
check "decision" "$(echo "$D4" | jq -r '.decision')" "ALLOW"

AUDIT_ID=$(echo "$D4" | jq -r '.auditId')
echo "  > GET /v1/audits/$AUDIT_ID   (provenance: resolvedBy + the LLM proposal)"
A4=$(get "/v1/audits/$AUDIT_ID")
show "audit" "$A4"
check "audit.resolvedBy" "$(echo "$A4" | jq -r '.resolvedBy')" "LLM_VALIDATED"
check "audit records llmProposal.canonical" "$(echo "$A4" | jq -r '.llmProposal.canonical')" "age_over_18"
echo "  NOTE: AI mapped the paraphrase; the deterministic engine validated it;"
echo "        credentials were NEVER shown to the model."
echo

# ======================================================================
bold "SCENARIO 5 — Guardrails refusing to guess"
hr
Q5='{"subjectId":"'"$SUBJECT"'","question":"Can this user pilot a plane?"}'
echo "  > POST /v1/verification/questions   (out of scope for the closed set)"
echo "    $Q5"
HTTP5=$(curl -s -o /tmp/eudi_d5.json -w '%{http_code}' -X POST "$API/v1/verification/questions" \
        -H 'Content-Type: application/json' -d "$Q5")
D5=$(cat /tmp/eudi_d5.json)
show "response (HTTP $HTTP5)" "$D5"
check "http status" "$HTTP5" "422"
check "error" "$(echo "$D5" | jq -r '.error')" "UNRESOLVED_QUESTION"
check "supportedQuestions present" "$(echo "$D5" | jq -r '.supportedQuestions | length > 0')" "true"
echo "  NOTE: out-of-scope questions are refused, not approximated — by construction"
echo "        the AI cannot introduce a new predicate."
echo
# ======================================================================
bold "SCENARIO 6 — REAL SD-JWT VC: issue → minimal disclosure → verify → replay → tamper"
hr
echo "  > POST /v1/wallet/issue  (wallet gets a real ES256-signed PID SD-JWT, key-bound)"
I6=$(post /v1/wallet/issue '{"subjectId":"'"$SUBJECT"'"}')
show "response" "$I6"
check "credential issued" "$(echo "$I6" | jq -r '.credential')" "SD-JWT VC (PID)"

echo "  > POST /v1/verification/questions  (age check → nonce challenge)"
R6=$(post /v1/verification/questions '{"subjectId":"'"$SUBJECT"'","question":"Is this user over 18?"}')
RID6=$(echo "$R6" | jq -r '.requestId')
NONCE6=$(echo "$R6" | jq -r '.nonce')
REQEV6=$(echo "$R6" | jq -c '.requiredEvidence')

echo "  > POST /v1/wallet/present  (discloses birth_date ONLY; signs a KB-JWT)"
WP6=$(jq -n --arg s "$SUBJECT" --arg r "$RID6" --arg n "$NONCE6" --argjson e "$REQEV6" \
        '{subjectId:$s,requestId:$r,nonce:$n,requiredEvidence:$e}')
PRES6=$(post /v1/wallet/present "$WP6")

echo "  > POST /v1/verification/presentations  (REAL cryptographic verification)"
PBODY6=$(jq -n --arg r "$RID6" --argjson p "$PRES6" '{requestId:$r,presentation:$p}')
D6=$(post /v1/verification/presentations "$PBODY6")
show "response" "$D6"
check "decision" "$(echo "$D6" | jq -r '.decision')" "ALLOW"
check "verifiedClaims.age_over_18" "$(echo "$D6" | jq -r '.verifiedClaims.age_over_18')" "true"
check "disclosureCount (minimal)" "$(echo "$D6" | jq -r '.disclosureCount')" "1"
check "all verification checks ok" "$(echo "$D6" | jq -r '[.verificationChecks[].ok] | all')" "true"
check "no birth_date leaked" "$(echo "$D6" | grep -ci 'birth_date\|1984\|mustermann')" "0"
echo "  POINT AT: the credential proved age via a REAL signature + key binding,"
echo "            yet only the derived boolean crossed the boundary — no DOB, no name."
echo

echo "  > REPLAY the exact same SD-JWT VP → single-use nonce rejects it"
D6R=$(post /v1/verification/presentations "$PBODY6")
show "response" "$D6R"
check "decision" "$(echo "$D6R" | jq -r '.decision')" "REPLAY_DETECTED"

echo "  > TAMPER a disclosure on a fresh challenge → cryptographic check fails"
R6T=$(post /v1/verification/questions '{"subjectId":"'"$SUBJECT"'","question":"Is this user over 18?"}')
RID6T=$(echo "$R6T" | jq -r '.requestId'); NONCE6T=$(echo "$R6T" | jq -r '.nonce')
WP6T=$(jq -n --arg s "$SUBJECT" --arg r "$RID6T" --arg n "$NONCE6T" --argjson e "$REQEV6" \
        '{subjectId:$s,requestId:$r,nonce:$n,requiredEvidence:$e}')
PRES6T=$(post /v1/wallet/present "$WP6T")
VP=$(echo "$PRES6T" | jq -r '.sdJwtVp')
# flip the last char of the disclosure segment (field 2 of issuer~disclosure~kb)
IFS='~' read -ra SEG <<< "$VP"
d="${SEG[1]}"; last="${d: -1}"
if [ "$last" = "A" ]; then SEG[1]="${d%?}B"; else SEG[1]="${d%?}A"; fi
TVP=$(IFS='~'; echo "${SEG[*]}")
PREST=$(echo "$PRES6T" | jq --arg vp "$TVP" '.sdJwtVp=$vp')
PBODY6T=$(jq -n --arg r "$RID6T" --argjson p "$PREST" '{requestId:$r,presentation:$p}')
D6T=$(post /v1/verification/presentations "$PBODY6T")
show "response" "$D6T"
check "decision" "$(echo "$D6T" | jq -r '.decision')" "DENY"
check "failedCheck" "$(echo "$D6T" | jq -r '.failedCheck')" "UNKNOWN_DISCLOSURE"
echo "  POINT AT: a one-character tamper is caught by the disclosure-digest check."
echo


# ======================================================================
hr
if [[ "$FAILURES" -eq 0 ]]; then
  bold "ALL SCENARIOS PASSED ✅"
  exit 0
else
  bold "$FAILURES CHECK(S) FAILED ❌"
  exit 1
fi
