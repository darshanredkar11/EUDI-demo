package com.demo.eudi.error;

import java.util.List;

/** Uniform error body: { error, message, details } plus supportedQuestions + stage for 422. */
public record ErrorResponse(
        String error,
        String message,
        Object details,
        List<String> supportedQuestions,
        String stage) {
}
