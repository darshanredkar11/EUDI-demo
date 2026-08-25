package com.demo.eudi.model;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;

public record PresentationRequest(
        @NotBlank String requestId,
        @NotNull PresentationDto presentation) {
}
