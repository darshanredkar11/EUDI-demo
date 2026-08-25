package com.demo.eudi;

import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.ArgumentMatchers.isNull;
import static org.mockito.Mockito.when;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.demo.eudi.client.EngineClient;
import com.demo.eudi.client.EnginePlanResponse;
import com.demo.eudi.client.EngineResolveOutcome;
import com.demo.eudi.client.EngineResolveResponse;
import java.util.List;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MockMvc;

/**
 * Exercises the two-tier resolution with guardrails through the real
 * orchestration + StubLlmResolver, with the engine faked at the client seam.
 * This is the AI showcase: LLM proposes, engine (mock) validates or refuses.
 */
@SpringBootTest
@AutoConfigureMockMvc
class GuardrailOrchestrationTest {

    @Autowired
    MockMvc mvc;

    @MockBean
    EngineClient engine;

    private static final List<String> SUPPORTED =
            List.of("BANK_ACCOUNT_OPENING_V1", "age_over_18");

    @BeforeEach
    void setup() {
        // Any plan call returns a fixed age plan.
        when(engine.plan(any())).thenReturn(new EnginePlanResponse(
                "req-1", "age_over_18", "predicate",
                List.of("AGE_ATTESTATION"), "nonce-1", "2099-01-01T00:00:00Z", null, null));
    }

    @Test
    void tier1_registry_hit_reports_REGISTRY() throws Exception {
        when(engine.resolve(eq("Is this user over 18?"), isNull()))
                .thenReturn(new EngineResolveOutcome(
                        new EngineResolveResponse("age_over_18", "predicate", "REGISTRY"), List.of()));

        mvc.perform(post("/v1/verification/questions")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"subjectId\":\"user-123\",\"question\":\"Is this user over 18?\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.canonical").value("age_over_18"))
                .andExpect(jsonPath("$.resolvedBy").value("REGISTRY"))
                .andExpect(jsonPath("$.requiredEvidence[0]").value("AGE_ATTESTATION"));
    }

    @Test
    void scenario4_llm_paraphrase_is_validated() throws Exception {
        String q = "Is this customer an adult?";
        // Tier 1 miss, then engine validates the LLM-proposed canonical.
        when(engine.resolve(eq(q), isNull()))
                .thenReturn(new EngineResolveOutcome(null, SUPPORTED));
        when(engine.resolve(eq(q), eq("age_over_18")))
                .thenReturn(new EngineResolveOutcome(
                        new EngineResolveResponse("age_over_18", "predicate", "LLM_VALIDATED"), List.of()));

        mvc.perform(post("/v1/verification/questions")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"subjectId\":\"user-123\",\"question\":\"" + q + "\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.canonical").value("age_over_18"))
                .andExpect(jsonPath("$.resolvedBy").value("LLM_VALIDATED"));
    }

    @Test
    void scenario5_out_of_scope_is_refused_not_guessed() throws Exception {
        String q = "Can this user pilot a plane?";
        when(engine.resolve(eq(q), isNull()))
                .thenReturn(new EngineResolveOutcome(null, SUPPORTED));
        // Stub abstains -> engine.resolve(q, canonical) is NEVER called -> 422.

        mvc.perform(post("/v1/verification/questions")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"subjectId\":\"user-123\",\"question\":\"" + q + "\"}"))
                .andExpect(status().isUnprocessableEntity())
                .andExpect(jsonPath("$.error").value("UNRESOLVED_QUESTION"))
                .andExpect(jsonPath("$.supportedQuestions").isArray());
    }

    @Test
    void validation_error_on_blank_question() throws Exception {
        mvc.perform(post("/v1/verification/questions")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"subjectId\":\"user-123\",\"question\":\"\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.error").value("VALIDATION_ERROR"));
    }
}
