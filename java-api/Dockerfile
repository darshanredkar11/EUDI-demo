# Java thin API + orchestration — multi-stage build (Java 21).
FROM maven:3.9-eclipse-temurin-21 AS build
WORKDIR /app
COPY pom.xml ./
RUN mvn -q -B dependency:go-offline || true
COPY src ./src
RUN mvn -q -B package -DskipTests

FROM eclipse-temurin:21-jre
WORKDIR /app
COPY --from=build /app/target/eudi-api-0.1.0.jar app.jar
ENV ENGINE_URL=http://engine:8081
EXPOSE 8080
CMD ["java", "-jar", "app.jar"]
