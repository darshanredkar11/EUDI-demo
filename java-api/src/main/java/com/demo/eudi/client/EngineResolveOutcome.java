package com.demo.eudi.client;

import java.util.List;

/**
 * Outcome of an engine resolve call. On a miss (HTTP 422) `resolution` is null
 * and `supportedQuestions` carries the closed-set list for the 422 response.
 */
public record EngineResolveOutcome(EngineResolveResponse resolution, List<String> supportedQuestions) {
    public boolean resolved() {
        return resolution != null;
    }
}
