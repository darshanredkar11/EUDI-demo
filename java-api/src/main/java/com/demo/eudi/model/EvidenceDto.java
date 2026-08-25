package com.demo.eudi.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;

/** A piece of evidence: type + state. Never a raw attribute value. */
@JsonIgnoreProperties(ignoreUnknown = true)
public record EvidenceDto(String type, String state) {
}
