package com.demo.eudi.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import java.util.List;
import java.util.Map;

/**
 * A wallet presentation. `sdJwtVp` carries a REAL SD-JWT VC presentation
 * (issuer JWT + selected disclosures + holder-signed KB-JWT). The `evidence`/
 * `signatures` fields remain only for the legacy mock path.
 */
@JsonIgnoreProperties(ignoreUnknown = true)
public record PresentationDto(
        String requestId,
        String nonce,
        String audience,
        List<EvidenceDto> evidence,
        Map<String, Object> signatures,
        String sdJwtVp) {
}
