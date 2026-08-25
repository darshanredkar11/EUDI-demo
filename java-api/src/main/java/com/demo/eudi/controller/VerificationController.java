package com.demo.eudi.controller;

import com.demo.eudi.model.DecisionRequest;
import com.demo.eudi.model.DecisionResponse;
import com.demo.eudi.model.PresentationRequest;
import com.demo.eudi.model.PresentationResponse;
import com.demo.eudi.model.QuestionRequest;
import com.demo.eudi.model.QuestionResponse;
import com.demo.eudi.service.VerificationService;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.media.ExampleObject;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

/** Thin controllers: translate HTTP <-> service. No business logic. */
@RestController
@RequestMapping("/v1/verification")
public class VerificationController {

    private final VerificationService service;

    public VerificationController(VerificationService service) {
        this.service = service;
    }

    @Operation(
            tags = {"1-Question Resolution"},
            summary = "Resolve a natural-language question and get an evidence request",
            description = "Tier 1 registry match (response `tier1`: HIT|MISS); on a miss the guardrailed "
                    + "LLM proposes a canonical id which the engine independently re-validates against the "
                    + "closed set. Returns the request nonce + privacy-minimal required evidence, or 422 if "
                    + "out of scope (the 422 `stage` names the guardrail that stopped it).")
    @ApiResponses({
            @ApiResponse(responseCode = "200", description = "Resolved; evidence request returned. "
                    + "`resolvedBy`=REGISTRY (tier1=HIT) or LLM_VALIDATED (tier1=MISS)."),
            @ApiResponse(responseCode = "422", description = "UNRESOLVED_QUESTION. Body carries `supportedQuestions` "
                    + "and `stage`: TIER2_ABSTAIN | TIER2_LOW_CONFIDENCE | TIER2_TIMEOUT | ENGINE_MEMBERSHIP_REJECTED. "
                    + "Example: {\"error\":\"UNRESOLVED_QUESTION\",\"stage\":\"TIER2_ABSTAIN\","
                    + "\"supportedQuestions\":[\"BANK_ACCOUNT_OPENING_V1\",\"age_over_18\"]}")
    })
    @io.swagger.v3.oas.annotations.parameters.RequestBody(content = @Content(examples = {
            @ExampleObject(name = "age (registry)", value = "{\"subjectId\":\"user-123\",\"question\":\"Is this user over 18?\"}"),
            @ExampleObject(name = "paraphrase (LLM)", value = "{\"subjectId\":\"user-123\",\"question\":\"Is this customer an adult?\"}"),
            @ExampleObject(name = "bank policy", value = "{\"subjectId\":\"user-123\",\"question\":\"Can this user open a bank account?\"}"),
            @ExampleObject(name = "out of scope", value = "{\"subjectId\":\"user-123\",\"question\":\"Can this user pilot a plane?\"}")
    }))
    @PostMapping("/questions")
    public QuestionResponse questions(@Valid @RequestBody QuestionRequest req) {
        return service.questions(req);
    }

    @Operation(
            tags = {"3-Presentation & Verification"},
            summary = "Verify a wallet presentation (real SD-JWT) and decide",
            description = "Verifies the SD-JWT VP (issuer signature, disclosure digests, validity, key "
                    + "binding, nonce) then evaluates the policy. Single-use nonce → a replayed VP returns "
                    + "REPLAY_DETECTED; a tampered VP returns DENY with the failing check named.")
    @ApiResponses({
            @ApiResponse(responseCode = "200", description = "ALLOW / DENY / UNKNOWN / REPLAY_DETECTED")
    })
    @io.swagger.v3.oas.annotations.parameters.RequestBody(content = @Content(examples = {
            @ExampleObject(name = "SD-JWT VP", value =
                    "{\"requestId\":\"<from /questions>\",\"presentation\":{\"sdJwtVp\":\"<from /v1/wallet/present>\"}}")
    }))
    @PostMapping("/presentations")
    public PresentationResponse presentations(@Valid @RequestBody PresentationRequest req) {
        return service.presentations(req);
    }

    @Operation(
            tags = {"3-Presentation & Verification"},
            summary = "Evaluate a policy decision from the subject's held evidence",
            description = "Config-driven policy evaluation. UNKNOWN is first-class and returns an "
                    + "evidenceRequestPlan for the missing predicates.")
    @io.swagger.v3.oas.annotations.parameters.RequestBody(content = @Content(examples = {
            @ExampleObject(name = "bank policy", value = "{\"subjectId\":\"user-123\",\"policy\":\"Can this user open a bank account?\"}"),
            @ExampleObject(name = "canonical id", value = "{\"subjectId\":\"user-123\",\"policy\":\"BANK_ACCOUNT_OPENING_V1\"}")
    }))
    @PostMapping("/decisions")
    public DecisionResponse decisions(@Valid @RequestBody DecisionRequest req) {
        return service.decisions(req);
    }
}
