package com.demo.eudi.resolution;

import com.fasterxml.jackson.databind.JsonNode;

/**
 * The LLM's proposal. `canonical == null` means ABSTAIN. `raw` is the exact
 * proposal JSON, recorded verbatim in the audit (guardrail G5).
 */
public record LlmProposal(String canonical, Confidence confidence, String reason, JsonNode raw) {

    public boolean isHighConfidenceCandidate() {
        return canonical != null && !canonical.isBlank() && confidence == Confidence.HIGH;
    }
}
