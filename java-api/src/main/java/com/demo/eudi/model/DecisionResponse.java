package com.demo.eudi.model;

import com.fasterxml.jackson.databind.JsonNode;
import java.util.List;

public record DecisionResponse(
        String decision,
        String canonical,
        Integer policyVersion,
        List<String> satisfiedPredicates,
        List<String> missingPredicates,
        List<EvidenceDto> evidenceUsed,
        List<EvidenceDto> evidenceIgnored,
        List<String> reasons,
        JsonNode evidenceRequestPlan,
        String auditId,
        String resolvedBy,
        String tier1) {
}
