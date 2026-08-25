package com.demo.eudi.model;

import jakarta.validation.constraints.NotBlank;

/** Ask the MOCK wallet to obtain (issue) a real PID SD-JWT VC for a subject. */
public record WalletIssueRequest(@NotBlank String subjectId) {
}
