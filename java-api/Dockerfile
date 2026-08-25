# Java thin API + orchestration — multi-stage build (Java 21).
FROM maven:3.9-eclipse-temurin-21 AS build
WORKDIR /app
COPY pom.xml ./
RUN mvn -q -B dependency:go-offline || true
COPY src ./src
RUN mvn -q -B package -DskipTests

FROM eclipse-temurin:21-jre
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/eudi-api-0.1.0.jar app.jar
ENV ENGINE_URL=http://engine:8081
EXPOSE 8080
HEALTHCHECK --interval=5s --timeout=3s --retries=30 \
  CMD sh -c 'curl -f http://localhost:${PORT:-8080}/ || exit 1'
# Cap heap as a % of the container's actual memory limit (cgroup-aware,
# Java 10+) rather than an unbounded default — on a 512MB free-tier
# container the JVM's default heap sizing can exceed available RAM and
# get OOM-killed mid-run, causing exactly the "works, then 502s" pattern
# a memory-unaware default produces.
CMD ["java", "-XX:MaxRAMPercentage=70.0", "-XX:+UseContainerSupport", "-jar", "app.jar"]
