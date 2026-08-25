package com.demo.eudi.service;

import java.security.KeyPair;
import java.util.Map;

/**
 * A real credential held by the MOCK wallet: the issuer-signed SD-JWT, the
 * per-claim disclosures (base64url), and the holder's ES256 keypair used to sign
 * key-binding JWTs. Only the wallet ever holds the private key.
 */
public record WalletCredential(
        String sdJwt,
        Map<String, String> disclosuresByClaim,
        KeyPair holderKey) {
}
