package com.demo.eudi.error;

import com.demo.eudi.client.EngineException;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.MethodArgumentNotValidException;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

/**
 * Uniform error contract. Codes: UNRESOLVED_QUESTION, UNKNOWN_REQUEST,
 * EXPIRED_NONCE, REPLAY_DETECTED, ENGINE_UNAVAILABLE, VALIDATION_ERROR.
 */
@RestControllerAdvice
public class GlobalExceptionHandler {

    @ExceptionHandler(UnresolvedQuestionException.class)
    public ResponseEntity<ErrorResponse> unresolved(UnresolvedQuestionException e) {
        return ResponseEntity.status(e.status())
                .body(new ErrorResponse(e.code(), e.getMessage(), null, e.supportedQuestions(), e.stage()));
    }

    @ExceptionHandler(ApiException.class)
    public ResponseEntity<ErrorResponse> api(ApiException e) {
        return ResponseEntity.status(e.status())
                .body(new ErrorResponse(e.code(), e.getMessage(), e.details(), null, null));
    }

    @ExceptionHandler(EngineException.class)
    public ResponseEntity<ErrorResponse> engine(EngineException e) {
        HttpStatus status = switch (e.code()) {
            case "EXPIRED_NONCE" -> HttpStatus.GONE;
            case "UNKNOWN_REQUEST", "WRONG_REQUEST_ID" -> HttpStatus.NOT_FOUND;
            case "WRONG_AUDIENCE" -> HttpStatus.CONFLICT;
            case "UNRESOLVED_QUESTION" -> HttpStatus.UNPROCESSABLE_ENTITY;
            case "VALIDATION_ERROR" -> HttpStatus.BAD_REQUEST;
            case "ENGINE_UNAVAILABLE" -> HttpStatus.SERVICE_UNAVAILABLE;
            default -> HttpStatus.BAD_GATEWAY;
        };
        String code = e.status() == 503 ? "ENGINE_UNAVAILABLE" : e.code();
        return ResponseEntity.status(status)
                .body(new ErrorResponse(code, "engine returned an error", null, null, null));
    }

    @ExceptionHandler(MethodArgumentNotValidException.class)
    public ResponseEntity<ErrorResponse> validation(MethodArgumentNotValidException e) {
        String msg = e.getBindingResult().getFieldErrors().stream()
                .map(fe -> fe.getField() + " " + fe.getDefaultMessage())
                .findFirst()
                .orElse("invalid request");
        return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                .body(new ErrorResponse("VALIDATION_ERROR", msg, null, null, null));
    }

    @ExceptionHandler(Exception.class)
    public ResponseEntity<ErrorResponse> generic(Exception e) {
        return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                .body(new ErrorResponse("INTERNAL_ERROR", e.getMessage(), null, null, null));
    }
}
