package com.demo.eudi.controller;

import com.demo.eudi.model.PresentationDto;
import com.demo.eudi.model.WalletGrantRequest;
import com.demo.eudi.model.WalletIssueRequest;
import com.demo.eudi.model.WalletPresentRequest;
import com.demo.eudi.service.MockWallet;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.media.ExampleObject;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import java.util.Map;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

/** MOCK wallet endpoints. Not a certified wallet, but the crypto it performs is real. */
@RestController
@RequestMapping("/v1/wallet")
@Tag(name = "2-Wallet")
public class WalletController {

    private final MockWallet wallet;

    public WalletController(MockWallet wallet) {
        this.wallet = wallet;
    }

    @Operation(summary = "Issue a real PID SD-JWT VC into the wallet",
            description = "Generates a holder ES256 keypair, requests issuance from the engine issuer "
                    + "(bound via cnf), and stores the SD-JWT + disclosures. Idempotent per subject. "
                    + "Returns the FULL issued credential (`vc`) — this MOCK wallet exposes what it holds on purpose.")
    @ApiResponse(responseCode = "200", content = @Content(examples = {
            @ExampleObject(name = "issued VC", value = "{\"subjectId\":\"user-123\",\"wallet\":\"MOCK\","
                    + "\"credential\":\"SD-JWT VC (PID)\","
                    + "\"disclosableClaims\":[\"given_name\",\"family_name\",\"birth_date\",\"resident_country\"],"
                    + "\"vc\":{\"sdJwt\":\"eyJhbGciOiJFUzI1NiIsInR5cCI6ImRjK3NkLWp3dCIsImtpZCI6ImRlbW8taXNzdWVyLWtleS0xIn0.<payload>.<sig>\","
                    + "\"disclosures\":[{\"claim\":\"birth_date\",\"disclosure\":\"WyI8c2FsdD4iLCJiaXJ0aF9kYXRlIiwiMTk4NC0wMS0yNiJd\"}],"
                    + "\"combined\":\"<issuer-jwt>~<d1>~<d2>~<d3>~<d4>~\"}}")
    }))
    @io.swagger.v3.oas.annotations.parameters.RequestBody(content = @Content(examples = {
            @ExampleObject(name = "issue", value = "{\"subjectId\":\"user-123\"}")
    }))
    @PostMapping("/issue")
    public Map<String, Object> issue(@Valid @RequestBody WalletIssueRequest req) {
        return wallet.issue(req.subjectId());
    }

    @Operation(summary = "Build a minimal SD-JWT VP for a challenge",
            description = "Discloses ONLY the claims the evidence request requires (e.g. an age check "
                    + "discloses birth_date only) and signs a key-binding JWT over the presented combination.")
    @io.swagger.v3.oas.annotations.parameters.RequestBody(content = @Content(examples = {
            @ExampleObject(name = "present", value =
                    "{\"subjectId\":\"user-123\",\"requestId\":\"<from /questions>\",\"nonce\":\"<from /questions>\",\"requiredEvidence\":[\"AGE_ATTESTATION\"]}")
    }))
    @PostMapping("/present")
    public PresentationDto present(@Valid @RequestBody WalletPresentRequest req) {
        return wallet.present(req);
    }

    @Operation(summary = "Grant an (abstract) credential; also ensures a real PID is issued",
            description = "Used by the policy-decision scenario to acquire the residence credential.")
    @io.swagger.v3.oas.annotations.parameters.RequestBody(content = @Content(examples = {
            @ExampleObject(name = "grant residence", value = "{\"subjectId\":\"user-123\",\"evidenceType\":\"RESIDENCE_CREDENTIAL\"}")
    }))
    @PostMapping("/grant")
    public Map<String, Object> grant(@Valid @RequestBody WalletGrantRequest req) {
        wallet.grant(req.subjectId(), req.evidenceType());
        return Map.of("granted", req.evidenceType(), "subjectId", req.subjectId(), "wallet", "MOCK");
    }
}
