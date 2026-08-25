package com.demo.eudi.resolution;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.time.Duration;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.client.SimpleClientHttpRequestFactory;
import org.springframework.web.client.RestClient;

/**
 * Real Anthropic Messages API proposer. Enabled only when ANTHROPIC_API_KEY is
 * present. Temperature 0, strict-JSON instruction, closed-world prompt (G1),
 * PII-free input (G3). Any error/timeout/malformed output -> ABSTAIN, so the
 * engine fails closed (G6). The engine still re-validates every proposal (G2).
 */
public class AnthropicLlmResolver implements QuestionResolver {

    private static final Logger log = LoggerFactory.getLogger(AnthropicLlmResolver.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();

    private final RestClient client;
    private final String apiKey;
    private final String model;

    public AnthropicLlmResolver(String baseUrl, String apiKey, String model, int timeoutMs) {
        SimpleClientHttpRequestFactory f = new SimpleClientHttpRequestFactory();
        f.setConnectTimeout((int) Duration.ofMillis(timeoutMs).toMillis());
        f.setReadTimeout(timeoutMs);
        this.client = RestClient.builder().baseUrl(baseUrl).requestFactory(f).build();
        this.apiKey = apiKey;
        this.model = model;
    }

    @Override
    public LlmProposal propose(String question, List<CanonicalEntry> catalogue) {
        try {
            String prompt = buildPrompt(question, catalogue);
            Map<String, Object> body = Map.of(
                    "model", model,
                    "max_tokens", 256,
                    "temperature", 0,
                    "system", SYSTEM,
                    "messages", List.of(Map.of("role", "user", "content", prompt)));

            JsonNode resp = client.post().uri("/v1/messages")
                    .header("x-api-key", apiKey)
                    .header("anthropic-version", "2023-06-01")
                    .header("content-type", "application/json")
                    .body(body)
                    .retrieve()
                    .body(JsonNode.class);

            String text = resp.path("content").path(0).path("text").asText("");
            JsonNode parsed = MAPPER.readTree(extractJson(text));
            String canonical = parsed.hasNonNull("canonical") ? parsed.get("canonical").asText() : null;
            Confidence conf = parseConfidence(parsed.path("confidence").asText(""));
            String reason = parsed.path("reason").asText("");
            return new LlmProposal(canonical, conf, reason, parsed);
        } catch (Exception e) {
            // Fail closed: any error -> ABSTAIN. Never guess, never block the demo.
            log.warn("Anthropic resolver error, abstaining: {}", e.toString());
            return new LlmProposal(null, Confidence.LOW, "resolver error", null);
        }
    }

    @Override
    public String modelId() {
        return model;
    }

    private static final String SYSTEM =
            "You map a user's question to exactly one canonical identifier from a closed set, "
                    + "or abstain. Respond with STRICT JSON only, no prose, no markdown.";

    private static String buildPrompt(String question, List<CanonicalEntry> catalogue) {
        String cat = catalogue.stream()
                .map(c -> "- " + c.canonical() + ": " + c.description())
                .collect(Collectors.joining("\n"));
        return "Closed canonical set (choose exactly one id, or null to abstain):\n"
                + cat + "\n\n"
                + "Question: \"" + question + "\"\n\n"
                + "Respond with strict JSON exactly of the form:\n"
                + "{ \"canonical\": \"<one enumerated id>\" | null, "
                + "\"confidence\": \"HIGH|MEDIUM|LOW\", \"reason\": \"<short>\" }\n"
                + "Use HIGH only when you are certain the question is exactly that predicate/policy. "
                + "If the question is outside the set, respond with canonical null.";
    }

    private static String extractJson(String text) {
        int start = text.indexOf('{');
        int end = text.lastIndexOf('}');
        if (start >= 0 && end > start) {
            return text.substring(start, end + 1);
        }
        return text;
    }

    private static Confidence parseConfidence(String s) {
        try {
            return Confidence.valueOf(s.trim().toUpperCase());
        } catch (Exception e) {
            return Confidence.LOW;
        }
    }
}
