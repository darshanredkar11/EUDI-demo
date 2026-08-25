package com.demo.eudi.config;

import com.demo.eudi.resolution.AnthropicLlmResolver;
import com.demo.eudi.resolution.CanonicalEntry;
import com.demo.eudi.resolution.QuestionResolver;
import com.demo.eudi.resolution.StubLlmResolver;
import java.util.List;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

/**
 * Selects the Tier-2 proposer: the real Anthropic model when ANTHROPIC_API_KEY
 * is present, otherwise the deterministic stub. Same interface, same guardrails.
 */
@Configuration
public class ResolverConfig {

    private static final Logger log = LoggerFactory.getLogger(ResolverConfig.class);

    /**
     * The closed canonical catalogue shown to the LLM (ids + one-line
     * descriptions). This is the ONLY domain context the model ever receives.
     */
    @Bean
    public List<CanonicalEntry> canonicalCatalogue() {
        return List.of(
                new CanonicalEntry("age_over_18",
                        "Whether the user is over 18 years old (an age predicate)."),
                new CanonicalEntry("BANK_ACCOUNT_OPENING_V1",
                        "Whether the user is eligible to open a bank account (a multi-requirement policy)."));
    }

    @Bean
    public QuestionResolver questionResolver(
            @Value("${anthropic.api-key:}") String apiKey,
            @Value("${anthropic.model}") String model,
            @Value("${anthropic.base-url}") String baseUrl,
            @Value("${anthropic.timeout-ms}") int timeoutMs) {
        if (apiKey != null && !apiKey.isBlank()) {
            log.info("Tier 2 resolver: AnthropicLlmResolver (model={})", model);
            return new AnthropicLlmResolver(baseUrl, apiKey, model, timeoutMs);
        }
        log.info("Tier 2 resolver: StubLlmResolver (no ANTHROPIC_API_KEY; stubbed model, identical guardrails)");
        return new StubLlmResolver();
    }
}
