package com.demo.eudi.error;

import java.util.List;
import org.springframework.http.HttpStatus;

/** 422 when a question resolves to no member of the closed canonical set. */
public class UnresolvedQuestionException extends ApiException {
    private final List<String> supportedQuestions;
    /** Which guardrail stage stopped resolution: TIER2_ABSTAIN, TIER2_LOW_CONFIDENCE,
     *  TIER2_TIMEOUT, or ENGINE_MEMBERSHIP_REJECTED. Null for a plain Tier-1 miss with no Tier-2. */
    private final String stage;

    public UnresolvedQuestionException(List<String> supportedQuestions, String stage) {
        super(HttpStatus.UNPROCESSABLE_ENTITY, "UNRESOLVED_QUESTION",
                "The question could not be resolved to a supported predicate or policy.",
                null);
        this.supportedQuestions = supportedQuestions;
        this.stage = stage;
    }

    public List<String> supportedQuestions() {
        return supportedQuestions;
    }

    public String stage() {
        return stage;
    }
}
