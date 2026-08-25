package com.demo.eudi.client;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;

/** Engine /engine/resolve response. */
@JsonIgnoreProperties(ignoreUnknown = true)
public record EngineResolveResponse(String canonical, String kind, String resolvedBy) {
}
