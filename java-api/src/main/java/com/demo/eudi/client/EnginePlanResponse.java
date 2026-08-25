package com.demo.eudi.client;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.databind.JsonNode;
import java.util.List;

/** Engine /engine/plan response. */
@JsonIgnoreProperties(ignoreUnknown = true)
public record EnginePlanResponse(
        String requestId,
        String canonical,
        String kind,
        List<String> requiredEvidence,
        String nonce,
        String expiresAt,
        JsonNode challenge,
        JsonNode plan) {
}
