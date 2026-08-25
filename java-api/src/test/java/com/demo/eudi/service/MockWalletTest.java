package com.demo.eudi.service;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.when;

import com.demo.eudi.client.EngineClient;
import com.demo.eudi.client.EngineIssueResponse;
import com.demo.eudi.client.EngineIssueResponse.Disclosure;
import com.demo.eudi.crypto.JwtCrypto;
import com.demo.eudi.model.PresentationDto;
import com.demo.eudi.model.WalletPresentRequest;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

/** The wallet must disclose ONLY the claims the evidence request requires. */
class MockWalletTest {

    private static final ObjectMapper MAPPER = new ObjectMapper();

    @Test
    void claims_mapping_is_minimal_for_age() {
        assertThat(MockWallet.claimsToDisclose(List.of("AGE_ATTESTATION")))
                .containsExactly("birth_date");
    }

    @Test
    void present_discloses_only_birth_date_for_age_check() throws Exception {
        EngineClient engine = mock(EngineClient.class);
        EngineIssueResponse issued = new EngineIssueResponse(
                "aaa.bbb.ccc", // opaque issuer JWT (not verified in this wallet-side test)
                List.of(
                        new Disclosure("given_name", disclosure("given_name", "Erika")),
                        new Disclosure("family_name", disclosure("family_name", "Mustermann")),
                        new Disclosure("birth_date", disclosure("birth_date", "1984-01-26")),
                        new Disclosure("resident_country", disclosure("resident_country", "DE"))),
                "combined", "urn:eu.europa.ec.eudi:pid:1", "2099-01-01T00:00:00Z");
        when(engine.issue(any())).thenReturn(issued);

        MockWallet wallet = new MockWallet(engine);
        PresentationDto vp = wallet.present(new WalletPresentRequest(
                "user-123", "req-1", "nonce-1", "relying-party-demo", List.of("AGE_ATTESTATION")));

        List<String> disclosedClaims = disclosedClaimNames(vp.sdJwtVp());
        assertThat(disclosedClaims).containsExactly("birth_date");
        assertThat(disclosedClaims).doesNotContain("given_name", "family_name", "resident_country");

        // The VP ends with a key-binding JWT (three dot-separated segments).
        String[] parts = vp.sdJwtVp().split("~");
        String kb = parts[parts.length - 1];
        assertThat(kb.split("\\.")).hasSize(3);
    }

    private static String disclosure(String name, String value) throws Exception {
        byte[] salt = new byte[16];
        new java.security.SecureRandom().nextBytes(salt);
        String saltB64 = JwtCrypto.b64url(salt);
        byte[] arr = MAPPER.writeValueAsBytes(List.of(saltB64, name, value));
        return JwtCrypto.b64url(arr);
    }

    private static List<String> disclosedClaimNames(String vp) throws Exception {
        String[] parts = vp.split("~");
        List<String> names = new ArrayList<>();
        // index 0 = issuer JWT, last = KB-JWT, middle = disclosures
        for (int i = 1; i < parts.length - 1; i++) {
            if (parts[i].isEmpty()) {
                continue;
            }
            byte[] raw = JwtCrypto.b64urlDecode(parts[i]);
            JsonNode arr = MAPPER.readTree(new String(raw, StandardCharsets.UTF_8));
            names.add(arr.get(1).asText());
        }
        return names;
    }
}
