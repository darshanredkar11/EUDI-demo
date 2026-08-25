package com.demo.eudi.client;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import java.util.List;

/** Engine /engine/issuer/issue response: an SD-JWT VC + its disclosures. */
@JsonIgnoreProperties(ignoreUnknown = true)
public record EngineIssueResponse(
        String sdJwt,
        List<Disclosure> disclosures,
        String combined,
        String vct,
        String expiresAt) {

    @JsonIgnoreProperties(ignoreUnknown = true)
    public record Disclosure(String claim, String disclosure) {
    }
}
