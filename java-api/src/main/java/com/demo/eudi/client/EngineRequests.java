package com.demo.eudi.client;

import com.demo.eudi.model.EvidenceDto;
import com.demo.eudi.model.PresentationDto;
import com.fasterxml.jackson.databind.JsonNode;
import java.util.List;

/** Explicit engine request DTOs (Java -> Rust engine contract). */
public final class EngineRequests {
    private EngineRequests() {
    }

    public record ResolveReq(String question, String proposedCanonical) {
    }

    public record IssueReq(String subjectId, Object holderJwk) {
    }

    public record PlanReq(
            String canonical,
            String subjectId,
            List<EvidenceDto> knownEvidence,
            String audience,
            String resolvedBy,
            JsonNode llmProposal,
            String question) {
    }

    public record EvaluateReq(
            String canonical,
            List<EvidenceDto> evidence,
            String evaluationTime,
            String question,
            String resolvedBy,
            JsonNode llmProposal) {
    }

    public record PresentReq(
            String requestId,
            PresentationDto presentation,
            String evaluationTime,
            String question,
            String resolvedBy,
            JsonNode llmProposal) {
    }
}
