package com.demo.eudi.resolution;

import static org.assertj.core.api.Assertions.assertThat;

import java.util.List;
import org.junit.jupiter.api.Test;

/** The stub obeys the closed-world contract: HIGH only for known paraphrases, else ABSTAIN. */
class StubLlmResolverTest {

    private final StubLlmResolver resolver = new StubLlmResolver();
    private final List<CanonicalEntry> catalogue = List.of(
            new CanonicalEntry("age_over_18", "over 18"),
            new CanonicalEntry("BANK_ACCOUNT_OPENING_V1", "bank account"));

    @Test
    void known_paraphrase_maps_high() {
        LlmProposal p = resolver.propose("Is this customer an adult?", catalogue);
        assertThat(p.canonical()).isEqualTo("age_over_18");
        assertThat(p.confidence()).isEqualTo(Confidence.HIGH);
        assertThat(p.isHighConfidenceCandidate()).isTrue();
    }

    @Test
    void unknown_question_abstains() {
        LlmProposal p = resolver.propose("Can this user pilot a plane?", catalogue);
        assertThat(p.canonical()).isNull();
        assertThat(p.isHighConfidenceCandidate()).isFalse();
    }
}
