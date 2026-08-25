package com.demo.eudi.model;

import jakarta.validation.constraints.NotBlank;

public record WalletGrantRequest(
        @NotBlank String subjectId,
        @NotBlank String evidenceType) {
}
