package com.demo.eudi.config;

import io.swagger.v3.oas.models.OpenAPI;
import io.swagger.v3.oas.models.info.Info;
import io.swagger.v3.oas.models.tags.Tag;
import java.util.List;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

/** Swagger UI at /swagger-ui.html, spec at /v3/api-docs. Tags ordered by flow. */
@Configuration
public class OpenApiConfig {

    @Bean
    public OpenAPI eudiOpenApi() {
        return new OpenAPI()
                .info(new Info()
                        .title("EUDI Evidence Inference API (Java orchestration)")
                        .version("0.2.0")
                        .description("""
                                Thin API + orchestration over the deterministic Rust engine.
                                Flow: business question → two-tier resolution (registry + guardrailed LLM) →
                                reverse evidence inference → REAL SD-JWT VC presentation & verification →
                                deterministic decision. The LLM proposes; the engine disposes; the model
                                never sees credentials. Run every demo scenario from this page via
                                "Try it out". No ANTHROPIC_API_KEY required (StubLlmResolver)."""))
                .tags(List.of(
                        new Tag().name("1-Question Resolution").description("Natural-language question → canonical predicate/policy"),
                        new Tag().name("2-Wallet").description("MOCK wallet: issue a real PID SD-JWT VC, minimise disclosure, build a VP"),
                        new Tag().name("3-Presentation & Verification").description("Verify presentations and evaluate policy decisions"),
                        new Tag().name("4-Audit").description("Provenance: decision reasons, verification checks, no PII")));
    }
}
