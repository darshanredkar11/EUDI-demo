package com.demo.eudi.service;

import com.demo.eudi.client.EngineClient;
import com.demo.eudi.client.EngineIssueResponse;
import com.demo.eudi.client.EngineRequests;
import com.demo.eudi.crypto.JwtCrypto;
import com.demo.eudi.model.EvidenceDto;
import com.demo.eudi.model.PresentationDto;
import com.demo.eudi.model.WalletPresentRequest;
import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.interfaces.ECPublicKey;
import java.time.Instant;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

/**
 * MOCK wallet — now holding a REAL SD-JWT VC.
 *
 * On issuance it generates its own ES256 holder keypair, requests a PID SD-JWT
 * from the engine issuer (bound to the holder key via cnf), and stores the
 * SD-JWT plus all disclosures. On presentation it performs DATA MINIMISATION
 * here: it discloses ONLY the claims the evidence request needs, and signs a
 * key-binding JWT over the presented combination with the holder key.
 *
 * Still "mock" because it is not a certified wallet (no secure element, no
 * wallet attestation) — but the cryptography it performs is real.
 *
 * The abstract-holdings map (for the policy `decisions` path) is unchanged.
 */
@Component
public class MockWallet {

    private static final Logger log = LoggerFactory.getLogger(MockWallet.class);
    private static final String DEFAULT_AUDIENCE = "relying-party-demo";

    private final EngineClient engine;
    /** Abstract credential holdings for the config-driven policy decisions path. */
    private final Map<String, Set<String>> holdings = new ConcurrentHashMap<>();
    /** Real SD-JWT credentials for the presentation path. */
    private final Map<String, WalletCredential> credentials = new ConcurrentHashMap<>();

    public MockWallet(EngineClient engine) {
        this.engine = engine;
        holdings.put("user-123", ConcurrentHashMap.newKeySet());
        holdings.get("user-123").addAll(List.of(
                "GOVERNMENT_IDENTITY", "AGE_ATTESTATION", "CONSENT_RECORD"));
    }

    // ---- policy decisions path (unchanged, abstract evidence) --------------

    public List<EvidenceDto> knownEvidence(String subjectId) {
        List<EvidenceDto> out = new ArrayList<>();
        for (String t : heldFor(subjectId)) {
            out.add(new EvidenceDto(t, "AVAILABLE"));
        }
        out.sort((a, b) -> a.type().compareTo(b.type()));
        return out;
    }

    /** Grant an abstract credential (Scenario 2) AND ensure a real PID is issued. */
    public void grant(String subjectId, String evidenceType) {
        heldFor(subjectId).add(evidenceType);
        ensureIssued(subjectId); // grant triggers issuance of the real credential
    }

    private Set<String> heldFor(String subjectId) {
        return holdings.computeIfAbsent(subjectId, k -> ConcurrentHashMap.newKeySet());
    }

    // ---- real SD-JWT presentation path -------------------------------------

    /** Obtain (issue) a real PID SD-JWT VC for the subject if not already held. */
    public synchronized Map<String, Object> issue(String subjectId) {
        WalletCredential c = ensureIssued(subjectId);
        Map<String, Object> summary = new LinkedHashMap<>();
        summary.put("subjectId", subjectId);
        summary.put("wallet", "MOCK");
        summary.put("credential", "SD-JWT VC (PID)");
        summary.put("disclosableClaims", new ArrayList<>(c.disclosuresByClaim().keySet()));
        // Full, UNREDACTED credential as issued (before any minimisation). This is a
        // MOCK wallet whose purpose is to demonstrate real crypto — exposing the
        // credential it holds is the point of the demo, not a leak.
        summary.put("vc", fullVc(c));
        return summary;
    }

    /** The complete issued SD-JWT VC: issuer JWT + every disclosure + combined string. */
    private static Map<String, Object> fullVc(WalletCredential c) {
        List<Map<String, String>> disclosures = new ArrayList<>();
        StringBuilder combined = new StringBuilder(c.sdJwt());
        for (Map.Entry<String, String> e : c.disclosuresByClaim().entrySet()) {
            disclosures.add(Map.of("claim", e.getKey(), "disclosure", e.getValue()));
            combined.append('~').append(e.getValue());
        }
        combined.append('~');
        Map<String, Object> vc = new LinkedHashMap<>();
        vc.put("sdJwt", c.sdJwt());
        vc.put("disclosures", disclosures);
        vc.put("combined", combined.toString());
        return vc;
    }

    private synchronized WalletCredential ensureIssued(String subjectId) {
        WalletCredential existing = credentials.get(subjectId);
        if (existing != null) {
            return existing;
        }
        KeyPair holder = JwtCrypto.generateEcKeyPair();
        Map<String, Object> holderJwk = JwtCrypto.publicJwk((ECPublicKey) holder.getPublic());
        EngineIssueResponse resp = engine.issue(new EngineRequests.IssueReq(subjectId, holderJwk));
        Map<String, String> byClaim = new LinkedHashMap<>();
        for (EngineIssueResponse.Disclosure d : resp.disclosures()) {
            byClaim.put(d.claim(), d.disclosure());
        }
        WalletCredential cred = new WalletCredential(resp.sdJwt(), byClaim, holder);
        credentials.put(subjectId, cred);
        log.info("MOCK wallet issued PID for {} ({} disclosable claims)", subjectId, byClaim.size());
        return cred;
    }

    /**
     * Which SD-JWT claims must be disclosed for a set of required evidence types.
     * Data minimisation: an age check maps to `birth_date` ONLY.
     */
    public static Set<String> claimsToDisclose(List<String> requiredEvidence) {
        Set<String> claims = new LinkedHashSet<>();
        for (String et : requiredEvidence) {
            switch (et) {
                case "AGE_ATTESTATION" -> claims.add("birth_date");
                case "GOVERNMENT_IDENTITY" -> {
                    claims.add("given_name");
                    claims.add("family_name");
                }
                case "RESIDENCE_CREDENTIAL" -> claims.add("resident_country");
                default -> { /* no PID claim for this evidence type */ }
            }
        }
        return claims;
    }

    /** Build a real SD-JWT VP disclosing ONLY the required claims + a KB-JWT. */
    public PresentationDto present(WalletPresentRequest req) {
        WalletCredential cred = ensureIssued(req.subjectId());
        String audience = req.audience() == null ? DEFAULT_AUDIENCE : req.audience();

        Set<String> toDisclose = claimsToDisclose(req.requiredEvidence());
        List<String> selected = new ArrayList<>();
        List<String> withheld = new ArrayList<>();
        for (Map.Entry<String, String> e : cred.disclosuresByClaim().entrySet()) {
            if (toDisclose.contains(e.getKey())) {
                selected.add(e.getValue());
            } else {
                withheld.add(e.getKey());
            }
        }
        log.info("MOCK wallet minimised disclosure: disclosing {} withholding {}",
                toDisclose, withheld);

        // presented = <issuer-jwt> ~ <selected-disclosure>* ~
        StringBuilder presented = new StringBuilder(cred.sdJwt());
        for (String d : selected) {
            presented.append('~').append(d);
        }
        presented.append('~');

        String sdHash = JwtCrypto.sha256Base64Url(presented.toString().getBytes(StandardCharsets.UTF_8));
        Map<String, Object> kbHeader = new LinkedHashMap<>();
        kbHeader.put("alg", "ES256");
        kbHeader.put("typ", "kb+jwt");
        Map<String, Object> kbPayload = new LinkedHashMap<>();
        kbPayload.put("iat", Instant.now().getEpochSecond());
        kbPayload.put("aud", audience);
        kbPayload.put("nonce", req.nonce());
        kbPayload.put("sd_hash", sdHash);
        String kbJwt = JwtCrypto.signEs256(cred.holderKey().getPrivate(), kbHeader, kbPayload);

        String vp = presented + kbJwt;
        return new PresentationDto(
                req.requestId(),
                req.nonce(),
                audience,
                List.of(),
                Map.of("wallet", "MOCK", "keyBinding", "ES256"),
                vp);
    }

    // test seam: inject a credential without contacting the engine
    void injectCredentialForTest(String subjectId, WalletCredential cred) {
        credentials.put(subjectId, cred);
    }
}
