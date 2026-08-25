package com.demo.eudi.config;

import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.http.client.SimpleClientHttpRequestFactory;
import org.springframework.web.client.RestClient;

/** RestClient wired to the Rust engine. */
@Configuration
public class RestClientConfig {

    @Bean
    public RestClient engineRestClient(@Value("${engine.base-url}") String baseUrl) {
        // Some hosting platforms' service-discovery env vars yield a bare
        // host:port with no scheme (e.g. Render's private-service
        // fromService reference); tolerate that instead of requiring every
        // deployment target to hand back a fully-qualified URL.
        String resolved = baseUrl.contains("://") ? baseUrl : "http://" + baseUrl;
        SimpleClientHttpRequestFactory factory = new SimpleClientHttpRequestFactory();
        factory.setConnectTimeout(2000);
        factory.setReadTimeout(5000);
        return RestClient.builder()
                .baseUrl(resolved)
                .requestFactory(factory)
                .build();
    }
}
