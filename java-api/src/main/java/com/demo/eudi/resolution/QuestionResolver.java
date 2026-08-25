package com.demo.eudi.resolution;

import java.util.List;

/**
 * Tier 2 proposer. The implementation (real LLM or stub) is an UNTRUSTED
 * PROPOSER outside the decision core. It maps a paraphrased question to ONE
 * member of the closed canonical set, or ABSTAINs. It receives ONLY the raw
 * question and the catalogue (ids + descriptions) — never subjectId,
 * credentials, VC/VP contents, evidence states, or prior answers (guardrail G3,
 * enforced structurally by this signature).
 */
public interface QuestionResolver {

    /** @return a proposal, always non-null; ABSTAIN is a proposal with null canonical. */
    LlmProposal propose(String question, List<CanonicalEntry> catalogue);

    /** Identifier recorded in the audit (e.g. model id or "stub-...."). */
    String modelId();
}
