package com.demo.eudi.model;

import java.util.List;

public record QuestionResponse(
        String requestId,
        String canonical,
        String kind,
        List<String> requiredEvidence,
        String nonce,
        String expiresAt,
        String resolvedBy,
        String tier1) {
}
