package com.demo.eudi.resolution;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ObjectNode;
import java.util.Map;

/**
 * Deterministic paraphrase table standing in for the model when no
 * ANTHROPIC_API_KEY is set. The SAME guardrails (G1-G6) and validation pipeline
 * apply; only the model call is substituted. Honest label for the demo:
 * "stubbed model, identical guardrail pipeline".
 *
 * Unknown paraphrases ABSTAIN — the stub cannot invent a canonical outside the
 * closed set, exactly like the real model under its closed-world prompt.
 */
public class StubLlmResolver implements QuestionResolver {

    private static final ObjectMapper MAPPER = new ObjectMapper();

    // Paraphrase -> canonical. Only HIGH-confidence, closed-set members.
    private static final Map<String, String> TABLE = Map.of(
            "is this customer an adult", "age_over_18",
            "is the customer an adult", "age_over_18",
            "is this customer of legal age", "age_over_18",
            "is the applicant an adult", "age_over_18",
            "can this customer open an account", "BANK_ACCOUNT_OPENING_V1",
            "is this customer eligible for a bank account", "BANK_ACCOUNT_OPENING_V1");

    @Override
    public LlmProposal propose(String question, java.util.List<CanonicalEntry> catalogue) {
        String norm = normalize(question);
        String canonical = TABLE.get(norm);
        if (canonical != null) {
            return new LlmProposal(canonical, Confidence.HIGH,
                    "stub paraphrase match", rawJson(canonical, "HIGH", "stub paraphrase match"));
        }
        // ABSTAIN
        return new LlmProposal(null, Confidence.LOW,
                "out of scope for the closed canonical set",
                rawJson(null, "LOW", "out of scope for the closed canonical set"));
    }

    @Override
    public String modelId() {
        return "stub-paraphrase-v1";
    }

    private static String normalize(String s) {
        return s.toLowerCase().trim().replaceAll("\\s+", " ").replaceAll("[?.!]+$", "").trim();
    }

    private static ObjectNode rawJson(String canonical, String confidence, String reason) {
        ObjectNode n = MAPPER.createObjectNode();
        if (canonical == null) {
            n.putNull("canonical");
        } else {
            n.put("canonical", canonical);
        }
        n.put("confidence", confidence);
        n.put("reason", reason);
        return n;
    }
}
