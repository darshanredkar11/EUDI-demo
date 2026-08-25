package com.demo.eudi.service;

import com.fasterxml.jackson.databind.JsonNode;

/** Result of the two-tier resolution: canonical, how it was resolved, and the Tier-1 outcome. */
public record Resolution(String canonical, String kind, String resolvedBy, JsonNode llmProposal, String tier1) {
}
