package com.demo.eudi;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

/**
 * Thin API + orchestration layer. All identity/policy business logic lives in
 * the Rust engine; this service only translates the external API to the engine
 * contract and orchestrates the two-tier question resolution (Tier 2 = LLM).
 */
@SpringBootApplication
public class EudiApiApplication {
    public static void main(String[] args) {
        SpringApplication.run(EudiApiApplication.class, args);
    }
}
