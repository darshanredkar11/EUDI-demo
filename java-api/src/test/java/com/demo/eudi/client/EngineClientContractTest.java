package com.demo.eudi.client;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.client.match.MockRestRequestMatchers.requestTo;
import static org.springframework.test.web.client.response.MockRestResponseCreators.withStatus;
import static org.springframework.test.web.client.response.MockRestResponseCreators.withSuccess;

import com.demo.eudi.client.EngineRequests.EvaluateReq;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.springframework.http.HttpStatus;
import org.springframework.http.MediaType;
import org.springframework.test.web.client.MockRestServiceServer;
import org.springframework.web.client.RestClient;

/**
 * Happy-path (and 422) contract test against the engine, using
 * MockRestServiceServer bound to the RestClient — no live engine, fully offline.
 */
class EngineClientContractTest {

    private EngineClient clientWith(MockServer[] holder) {
        RestClient.Builder builder = RestClient.builder().baseUrl("http://engine");
        MockRestServiceServer server = MockRestServiceServer.bindTo(builder).build();
        holder[0] = new MockServer(server);
        return new EngineClient(builder.build());
    }

    private record MockServer(MockRestServiceServer server) {
    }

    @Test
    void resolve_hit_maps_response() {
        MockServer[] h = new MockServer[1];
        EngineClient client = clientWith(h);
        h[0].server().expect(requestTo("http://engine/engine/resolve"))
                .andRespond(withSuccess(
                        "{\"canonical\":\"age_over_18\",\"kind\":\"predicate\",\"resolvedBy\":\"REGISTRY\"}",
                        MediaType.APPLICATION_JSON));

        EngineResolveOutcome out = client.resolve("is this user over 18", null);
        assertThat(out.resolved()).isTrue();
        assertThat(out.resolution().canonical()).isEqualTo("age_over_18");
        assertThat(out.resolution().resolvedBy()).isEqualTo("REGISTRY");
        h[0].server().verify();
    }

    @Test
    void resolve_422_returns_supported_questions() {
        MockServer[] h = new MockServer[1];
        EngineClient client = clientWith(h);
        h[0].server().expect(requestTo("http://engine/engine/resolve"))
                .andRespond(withStatus(HttpStatus.UNPROCESSABLE_ENTITY)
                        .contentType(MediaType.APPLICATION_JSON)
                        .body("{\"error\":\"UNRESOLVED_QUESTION\",\"supportedQuestions\":[\"age_over_18\"]}"));

        EngineResolveOutcome out = client.resolve("pilot a plane", null);
        assertThat(out.resolved()).isFalse();
        assertThat(out.supportedQuestions()).containsExactly("age_over_18");
        h[0].server().verify();
    }

    @Test
    void evaluate_maps_decision() {
        MockServer[] h = new MockServer[1];
        EngineClient client = clientWith(h);
        h[0].server().expect(requestTo("http://engine/engine/evaluate"))
                .andRespond(withSuccess(
                        "{\"decision\":\"ALLOW\",\"canonical\":\"age_over_18\",\"kind\":\"predicate\","
                                + "\"predicate\":\"age_over_18\",\"verifiedClaims\":{\"age_over_18\":true},"
                                + "\"auditId\":\"a-1\"}",
                        MediaType.APPLICATION_JSON));

        EngineDecisionResponse dec = client.evaluate(new EvaluateReq(
                "age_over_18", List.of(), null, "q", "REGISTRY", null));
        assertThat(dec.decision()).isEqualTo("ALLOW");
        assertThat(dec.verifiedClaims()).containsEntry("age_over_18", true);
        assertThat(dec.auditId()).isEqualTo("a-1");
        h[0].server().verify();
    }
}
