package com.demo.eudi.error;

import org.springframework.http.HttpStatus;

/** Uniform error contract carrier: code + status + optional details. */
public class ApiException extends RuntimeException {
    private final String code;
    private final HttpStatus status;
    private final transient Object details;

    public ApiException(HttpStatus status, String code, String message, Object details) {
        super(message);
        this.status = status;
        this.code = code;
        this.details = details;
    }

    public String code() {
        return code;
    }

    public HttpStatus status() {
        return status;
    }

    public Object details() {
        return details;
    }
}
