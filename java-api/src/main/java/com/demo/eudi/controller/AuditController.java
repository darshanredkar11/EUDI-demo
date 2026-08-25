package com.demo.eudi.controller;

import com.demo.eudi.client.EngineClient;
import com.fasterxml.jackson.databind.JsonNode;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.tags.Tag;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RestController;

/** Passthrough to the engine's audit log so the demo can show provenance. */
@RestController
@Tag(name = "4-Audit")
public class AuditController {

    private final EngineClient engine;

    public AuditController(EngineClient engine) {
        this.engine = engine;
    }

    @Operation(summary = "Fetch an audit record by id",
            description = "Provenance without PII: decision, predicate states, verification check results, "
                    + "disclosure count, and (for LLM-assisted resolutions) the model proposal.")
    @GetMapping("/v1/audits/{auditId}")
    public JsonNode audit(@PathVariable String auditId) {
        return engine.audit(auditId);
    }
}
