package com.demo.eudi.crypto;

import com.fasterxml.jackson.databind.ObjectMapper;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.MessageDigest;
import java.security.PrivateKey;
import java.security.Signature;
import java.security.interfaces.ECPublicKey;
import java.security.spec.ECGenParameterSpec;
import java.security.spec.ECPoint;
import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Minimal real ES256 (P-256) JOSE helpers for the holder wallet: EC keygen,
 * public JWK, compact JWS signing (raw R||S), SHA-256 and base64url.
 *
 * This is genuine cryptography (java.security), not a mock — the KB-JWT it
 * produces is verified by the Rust engine against the credential's cnf key.
 */
public final class JwtCrypto {

    private static final ObjectMapper MAPPER = new ObjectMapper();
    private static final Base64.Encoder B64URL = Base64.getUrlEncoder().withoutPadding();
    private static final Base64.Decoder B64URL_DEC = Base64.getUrlDecoder();

    private JwtCrypto() {
    }

    public static KeyPair generateEcKeyPair() {
        try {
            KeyPairGenerator kpg = KeyPairGenerator.getInstance("EC");
            kpg.initialize(new ECGenParameterSpec("secp256r1"));
            return kpg.generateKeyPair();
        } catch (Exception e) {
            throw new IllegalStateException("EC keygen failed", e);
        }
    }

    /** Public EC P-256 JWK: {kty, crv, x, y}. */
    public static Map<String, Object> publicJwk(ECPublicKey pub) {
        ECPoint w = pub.getW();
        Map<String, Object> jwk = new LinkedHashMap<>();
        jwk.put("kty", "EC");
        jwk.put("crv", "P-256");
        jwk.put("x", b64url(fixed(w.getAffineX(), 32)));
        jwk.put("y", b64url(fixed(w.getAffineY(), 32)));
        return jwk;
    }

    /** Sign a compact JWS with ES256 (raw 64-byte signature). */
    public static String signEs256(PrivateKey key, Map<String, Object> header, Map<String, Object> payload) {
        try {
            String signingInput = b64url(json(header)) + "." + b64url(json(payload));
            Signature sig = Signature.getInstance("SHA256withECDSA");
            sig.initSign(key);
            sig.update(signingInput.getBytes(StandardCharsets.UTF_8));
            byte[] raw = derToRaw(sig.sign(), 32);
            return signingInput + "." + b64url(raw);
        } catch (Exception e) {
            throw new IllegalStateException("ES256 sign failed", e);
        }
    }

    public static String b64url(byte[] bytes) {
        return B64URL.encodeToString(bytes);
    }

    public static byte[] b64urlDecode(String s) {
        return B64URL_DEC.decode(s);
    }

    public static String sha256Base64Url(byte[] bytes) {
        try {
            return b64url(MessageDigest.getInstance("SHA-256").digest(bytes));
        } catch (Exception e) {
            throw new IllegalStateException("sha256 failed", e);
        }
    }

    private static byte[] json(Map<String, Object> m) {
        try {
            return MAPPER.writeValueAsBytes(m);
        } catch (Exception e) {
            throw new IllegalStateException("json encode failed", e);
        }
    }

    /** Left-pad/truncate an unsigned big-endian coordinate to `len` bytes. */
    private static byte[] fixed(BigInteger v, int len) {
        byte[] b = v.toByteArray();
        if (b.length == len) {
            return b;
        }
        byte[] out = new byte[len];
        if (b.length > len) {
            // strip leading sign/zero bytes
            System.arraycopy(b, b.length - len, out, 0, len);
        } else {
            System.arraycopy(b, 0, out, len - b.length, b.length);
        }
        return out;
    }

    /** Convert a DER-encoded ECDSA signature to fixed-width R||S (JOSE). */
    static byte[] derToRaw(byte[] der, int len) {
        int i = 0;
        if (der[i++] != 0x30) {
            throw new IllegalArgumentException("bad DER: no SEQUENCE");
        }
        // sequence length (short form for P-256 signatures)
        int seqLen = der[i++] & 0xff;
        if ((seqLen & 0x80) != 0) {
            // long form length: skip the extra length bytes
            int n = seqLen & 0x7f;
            i += n;
        }
        if (der[i++] != 0x02) {
            throw new IllegalArgumentException("bad DER: no R INTEGER");
        }
        int rLen = der[i++] & 0xff;
        byte[] r = Arrays.copyOfRange(der, i, i + rLen);
        i += rLen;
        if (der[i++] != 0x02) {
            throw new IllegalArgumentException("bad DER: no S INTEGER");
        }
        int sLen = der[i++] & 0xff;
        byte[] s = Arrays.copyOfRange(der, i, i + sLen);

        byte[] out = new byte[len * 2];
        copyFixed(r, out, 0, len);
        copyFixed(s, out, len, len);
        return out;
    }

    private static void copyFixed(byte[] src, byte[] dst, int off, int len) {
        if (src.length > len) {
            System.arraycopy(src, src.length - len, dst, off, len);
        } else {
            System.arraycopy(src, 0, dst, off + (len - src.length), src.length);
        }
    }
}
