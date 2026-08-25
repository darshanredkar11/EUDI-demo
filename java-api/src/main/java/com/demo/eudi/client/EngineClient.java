package com.demo.eudi.client;

import com.demo.eudi.client.EngineRequests.EvaluateReq;
import com.demo.eudi.client.EngineRequests.PlanReq;
import com.demo.eudi.client.EngineRequests.PresentReq;
import com.demo.eudi.client.EngineRequests.ResolveReq;
import com.fasterxml.jackson.databind.JsonNode;
import java.util.ArrayList;
import java.util.List;
import org.springframework.stereotype.Component;
import org.springframework.web.client.ResourceAccessException;
import org.springframework.web.client.RestClient;

/**
 * The single seam between the Java orchestration layer and the Rust engine.
 * Java never re-implements any engine rule; it only marshals DTOs.
 */
@Component
public class EngineClient {

    private final RestClient client;

    public EngineClient(RestClient engineRestClient) {
        this.client = engineRestClient;
    }

    /** Tier 1 (question only) or Tier 2 validation (with proposedCanonical). */
    public EngineResolveOutcome resolve(String question, String proposedCanonical) {
        ResolveReq req = new ResolveReq(question, proposedCanonical);
        return exchange("/engine/resolve", req, resp -> {
            if (resp.status.is2xxSuccessful()) {
                return new EngineResolveOutcome(
                        resp.as(EngineResolveResponse.class), List.of());
            }
            if (resp.status.value() == 422) {
                return new EngineResolveOutcome(null, supportedQuestions(resp.body()));
            }
            throw new EngineException(resp.status.value(), engineCode(resp.body()), resp.body());
        });
    }

    public EnginePlanResponse plan(PlanReq req) {
        return post("/engine/plan", req, EnginePlanResponse.class);
    }

    public EngineDecisionResponse evaluate(EvaluateReq req) {
        return post("/engine/evaluate", req, EngineDecisionResponse.class);
    }

    public EngineDecisionResponse present(PresentReq req) {
        return post("/engine/presentations", req, EngineDecisionResponse.class);
    }

    public EngineIssueResponse issue(EngineRequests.IssueReq req) {
        return post("/engine/issuer/issue", req, EngineIssueResponse.class);
    }

    public JsonNode audit(String auditId) {
        return get("/engine/audits/" + auditId);
    }

    // ---- internals --------------------------------------------------------

    private <T> T post(String path, Object body, Class<T> type) {
        return exchange(path, body, resp -> {
            if (resp.status.is2xxSuccessful()) {
                return resp.as(type);
            }
            throw new EngineException(resp.status.value(), engineCode(resp.body()), resp.body());
        });
    }

    private JsonNode get(String path) {
        try {
            return client.get().uri(path).exchange((req, resp) -> {
                JsonNode body = safeBody(resp);
                if (resp.getStatusCode().is2xxSuccessful()) {
                    return body;
                }
                throw new EngineException(resp.getStatusCode().value(), engineCode(body), body);
            });
        } catch (ResourceAccessException e) {
            throw new EngineException(503, "ENGINE_UNAVAILABLE", null);
        }
    }

    private <T> T exchange(String path, Object body, java.util.function.Function<Resp, T> fn) {
        try {
            return client.post().uri(path)
                    .contentType(org.springframework.http.MediaType.APPLICATION_JSON)
                    .body(body)
                    .exchange((req, resp) ->
                            fn.apply(new Resp(resp.getStatusCode(), safeBody(resp), resp)));
        } catch (ResourceAccessException e) {
            throw new EngineException(503, "ENGINE_UNAVAILABLE", null);
        }
    }

    private static JsonNode safeBody(RestClient.RequestHeadersSpec.ConvertibleClientHttpResponse resp) {
        try {
            return resp.bodyTo(JsonNode.class);
        } catch (Exception e) {
            return null;
        }
    }

    private static String engineCode(JsonNode body) {
        if (body != null && body.hasNonNull("error")) {
            return body.get("error").asText();
        }
        return "ENGINE_ERROR";
    }

    private static List<String> supportedQuestions(JsonNode body) {
        List<String> out = new ArrayList<>();
        if (body != null && body.has("supportedQuestions")) {
            body.get("supportedQuestions").forEach(n -> out.add(n.asText()));
        }
        return out;
    }

    /** Small holder so the lambda can read status + already-parsed body. */
    private record Resp(
            org.springframework.http.HttpStatusCode status,
            JsonNode body,
            RestClient.RequestHeadersSpec.ConvertibleClientHttpResponse raw) {
        <T> T as(Class<T> type) {
            return new com.fasterxml.jackson.databind.ObjectMapper()
                    .convertValue(body, type);
        }
    }
}
