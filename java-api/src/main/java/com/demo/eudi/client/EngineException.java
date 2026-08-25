package com.demo.eudi.client;

import com.fasterxml.jackson.databind.JsonNode;

/** Raised when the engine returns a 4xx/5xx. Carries the engine's error code. */
public class EngineException extends RuntimeException {
    private final int status;
    private final String code;
    private final JsonNode body;

    public EngineException(int status, String code, JsonNode body) {
        super("engine error " + status + ": " + code);
        this.status = status;
        this.code = code;
        this.body = body;
    }

    public int status() {
        return status;
    }

    public String code() {
        return code;
    }

    public JsonNode body() {
        return body;
    }
}
