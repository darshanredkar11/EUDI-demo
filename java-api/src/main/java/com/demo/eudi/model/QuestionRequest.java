package com.demo.eudi.model;

import jakarta.validation.constraints.NotBlank;

/** Raw natural-language question accepted here. */
public record QuestionRequest(
        @NotBlank String subjectId,
        @NotBlank String question) {
}
