package com.demo.eudi.client;

import com.demo.eudi.model.EvidenceDto;
import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.databind.JsonNode;
import java.util.List;
import java.util.Map;

/**
 * Engine decision body, shared by /engine/evaluate and /engine/presentations
 * (the latter adds `reason` on REPLAY_DETECTED). Java forwards these fields;
 * it never re-derives a decision.
 */
@JsonIgnoreProperties(ignoreUnknown = true)
public record EngineDecisionResponse(
        String decision,
        String canonical,
        String kind,
        String predicate,
        Integer policyVersion,
        Map<String, Boolean> verifiedClaims,
        List<String> satisfiedPredicates,
        List<String> missingPredicates,
        List<EvidenceDto> evidenceUsed,
        List<EvidenceDto> evidenceIgnored,
        List<String> reasons,
        String evaluationTime,
        JsonNode evidenceRequestPlan,
        String reason,
        String auditId,
        JsonNode verificationChecks,
        Integer disclosureCount,
        String failedCheck) {
}
