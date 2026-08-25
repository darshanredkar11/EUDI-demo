package com.demo.eudi.service;

import com.demo.eudi.client.EngineClient;
import com.demo.eudi.client.EngineDecisionResponse;
import com.demo.eudi.client.EnginePlanResponse;
import com.demo.eudi.client.EngineRequests.EvaluateReq;
import com.demo.eudi.client.EngineRequests.PlanReq;
import com.demo.eudi.client.EngineRequests.PresentReq;
import com.demo.eudi.client.EngineResolveOutcome;
import com.demo.eudi.error.UnresolvedQuestionException;
import com.demo.eudi.model.DecisionRequest;
import com.demo.eudi.model.DecisionResponse;
import com.demo.eudi.model.EvidenceDto;
import com.demo.eudi.model.PresentationRequest;
import com.demo.eudi.model.PresentationResponse;
import com.demo.eudi.model.QuestionRequest;
import com.demo.eudi.model.QuestionResponse;
import com.demo.eudi.resolution.CanonicalEntry;
import com.demo.eudi.resolution.LlmProposal;
import com.demo.eudi.resolution.QuestionResolver;
import com.fasterxml.jackson.databind.JsonNode;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;

/**
 * Orchestration only. Contains NO business policy logic. It sequences:
 * two-tier resolution (Tier 1 engine registry -> Tier 2 LLM propose + engine
 * validate), reverse-inference planning, and evaluation — all decided by the
 * engine. The LLM PROPOSES; the engine DISPOSES.
 */
@Service
public class VerificationService {

    private static final Logger log = LoggerFactory.getLogger(VerificationService.class);

    private final EngineClient engine;
    private final QuestionResolver resolver;
    private final List<CanonicalEntry> catalogue;
    private final MockWallet wallet;

    /** requestId -> resolution provenance, so presentation audits show resolvedBy. */
    private final Map<String, Resolution> provenanceByRequest = new ConcurrentHashMap<>();
    /** requestId -> raw question, so presentation audits record the original question. */
    private final Map<String, String> questionByRequest = new ConcurrentHashMap<>();

    public VerificationService(EngineClient engine, QuestionResolver resolver,
                               List<CanonicalEntry> catalogue, MockWallet wallet) {
        this.engine = engine;
        this.resolver = resolver;
        this.catalogue = catalogue;
        this.wallet = wallet;
    }

    /** Two-tier resolution. Throws UnresolvedQuestionException on refusal (fail closed). */
    public Resolution resolve(String question) {
        // Tier 1: deterministic registry (in the engine).
        EngineResolveOutcome tier1 = engine.resolve(question, null);
        if (tier1.resolved()) {
            var r = tier1.resolution();
            return new Resolution(r.canonical(), r.kind(), r.resolvedBy(), null, "HIT");
        }

        // Tier 2: LLM proposes (untrusted), engine validates membership.
        LlmProposal proposal = resolver.propose(question, catalogue); // G3: only question + catalogue
        if (!proposal.isHighConfidenceCandidate()) { // G2/G6: non-HIGH or ABSTAIN -> refuse
            String stage = tier2RefusalStage(proposal);
            log.info("Tier 2 refused ({}) for question='{}'", stage, question);
            throw new UnresolvedQuestionException(tier1.supportedQuestions(), stage);
        }
        EngineResolveOutcome tier2 = engine.resolve(question, proposal.canonical());
        if (!tier2.resolved()) { // engine rejected the proposed id -> refuse
            log.info("Engine rejected LLM proposal '{}' for question='{}'", proposal.canonical(), question);
            throw new UnresolvedQuestionException(tier2.supportedQuestions(), "ENGINE_MEMBERSHIP_REJECTED");
        }
        var r = tier2.resolution();
        log.info("Tier 2 validated: '{}' -> {} (model={})", question, r.canonical(), resolver.modelId());
        return new Resolution(r.canonical(), r.kind(), r.resolvedBy(), proposal.raw(), "MISS");
    }

    /** Classify a Tier-2 refusal (before the engine gate) into an audit-able stage. */
    private static String tier2RefusalStage(LlmProposal p) {
        String c = p.canonical();
        if (c == null || c.isBlank()) {
            String reason = p.reason() == null ? "" : p.reason().toLowerCase();
            return (reason.contains("error") || reason.contains("timeout")) ? "TIER2_TIMEOUT" : "TIER2_ABSTAIN";
        }
        return "TIER2_LOW_CONFIDENCE"; // canonical present but confidence != HIGH
    }

    public QuestionResponse questions(QuestionRequest req) {
        Resolution res = resolve(req.question());
        EnginePlanResponse plan = engine.plan(new PlanReq(
                res.canonical(), req.subjectId(), List.of(), null,
                res.resolvedBy(), res.llmProposal(), req.question()));
        provenanceByRequest.put(plan.requestId(), res);
        questionByRequest.put(plan.requestId(), req.question());
        return new QuestionResponse(
                plan.requestId(), res.canonical(), plan.kind(),
                plan.requiredEvidence(), plan.nonce(), plan.expiresAt(), res.resolvedBy(), res.tier1());
    }

    public PresentationResponse presentations(PresentationRequest req) {
        Resolution prov = provenanceByRequest.get(req.requestId());
        String resolvedBy = prov != null ? prov.resolvedBy() : null;
        JsonNode proposal = prov != null ? prov.llmProposal() : null;
        String question = questionByRequest.get(req.requestId());
        EngineDecisionResponse dec = engine.present(new PresentReq(
                req.requestId(), req.presentation(), null, question, resolvedBy, proposal));
        return new PresentationResponse(
                dec.decision(), dec.predicate(), dec.verifiedClaims(),
                dec.evidenceUsed(), dec.reason(), dec.failedCheck(),
                dec.verificationChecks(), dec.disclosureCount(), dec.auditId(), resolvedBy);
    }

    public DecisionResponse decisions(DecisionRequest req) {
        Resolution res = resolve(req.policy());
        List<EvidenceDto> known = wallet.knownEvidence(req.subjectId());
        EngineDecisionResponse dec = engine.evaluate(new EvaluateReq(
                res.canonical(), known, null, req.policy(), res.resolvedBy(), res.llmProposal()));
        return new DecisionResponse(
                dec.decision(), dec.canonical(), dec.policyVersion(),
                dec.satisfiedPredicates(), dec.missingPredicates(),
                dec.evidenceUsed(), dec.evidenceIgnored(), dec.reasons(),
                dec.evidenceRequestPlan(), dec.auditId(), res.resolvedBy(), res.tier1());
    }
}
