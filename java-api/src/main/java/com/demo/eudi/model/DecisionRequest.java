package com.demo.eudi.model;

import jakarta.validation.constraints.NotBlank;

/** `policy` may be a canonical policy id or a natural-language question. */
public record DecisionRequest(
        @NotBlank String subjectId,
        @NotBlank String policy) {
}
