package com.demo.eudi.model;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotEmpty;
import java.util.List;

/** Ask the MOCK wallet to build a presentation for a given challenge. */
public record WalletPresentRequest(
        @NotBlank String subjectId,
        @NotBlank String requestId,
        @NotBlank String nonce,
        String audience,
        @NotEmpty List<String> requiredEvidence) {
}
