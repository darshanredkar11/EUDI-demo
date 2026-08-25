package com.demo.eudi.model;

import com.fasterxml.jackson.databind.JsonNode;
import java.util.List;
import java.util.Map;

/** Nulls are omitted (Jackson NON_NULL): replay responses carry reason, not claims. */
public record PresentationResponse(
        String decision,
        String predicate,
        Map<String, Boolean> verifiedClaims,
        List<EvidenceDto> evidenceUsed,
        String reason,
        String failedCheck,
        JsonNode verificationChecks,
        Integer disclosureCount,
        String auditId,
        String resolvedBy) {
}
