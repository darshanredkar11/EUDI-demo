package com.demo.eudi.resolution;

/** LLM self-reported confidence. Only HIGH is ever accepted (guardrail G2). */
public enum Confidence {
    HIGH,
    MEDIUM,
    LOW
}
