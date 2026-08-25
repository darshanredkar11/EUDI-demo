# Expected outcomes (asserted by run_demo.sh)

| Scenario | Endpoint(s)                                   | Expected outcome |
|----------|-----------------------------------------------|------------------|
| 1        | questions → wallet/present → presentations    | `ALLOW`, `verifiedClaims.age_over_18=true`, `resolvedBy=REGISTRY`, no DOB in response |
| 2        | decisions → wallet/grant → decisions          | `UNKNOWN` with `evidenceRequestPlan=[RESIDENCE_CREDENTIAL]`, then `ALLOW` |
| 3        | presentations (replayed Scenario 1)           | `REPLAY_DETECTED`, `reason=NONCE_ALREADY_CONSUMED` |
| 4        | questions (paraphrase) → presentations → audit| `ALLOW`, `resolvedBy=LLM_VALIDATED`, audit records `llmProposal` |
| 5        | questions (out of scope)                       | HTTP `422 UNRESOLVED_QUESTION` + `supportedQuestions` |
