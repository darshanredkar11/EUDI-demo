\
# Run the EUDI demo without Docker: two local processes (Rust engine, Java API),
# managed by PID file + health check. Mirrors docker-compose.yml 1:1 — same
# ports, same env vars — Docker stays available as an alternative, not a
# replacement.
#
# Quick start:
#   make run     # builds (if needed) and starts both services
#   make ui      # opens the demo console in your browser
#   make demo    # runs the 6-scenario scripted demo against the running stack
#   make stop    # stops both services
#   make status  # shows what's running
#   make logs    # tails both service logs
#
# Platform support: macOS and Linux (including WSL2 on Windows) natively.
# Plain Windows (cmd/PowerShell, no WSL) has no `make` and no POSIX shell for
# these recipes — on Windows use Docker Desktop instead (docker-compose.yml
# works unmodified) or install WSL2 and run this Makefile from an Ubuntu
# shell. See `make help` for details.

SHELL := /bin/bash

RUN_DIR    := .run
ENGINE_LOG := $(RUN_DIR)/engine.log
API_LOG    := $(RUN_DIR)/api.log
ENGINE_PID := $(RUN_DIR)/engine.pid
API_PID    := $(RUN_DIR)/api.pid

ENGINE_URL := http://localhost:8081
API_URL    := http://localhost:8080

# JDK 21 detection: macOS via java_home; Linux/WSL via an already-correct
# JAVA_HOME, then a search of common JVM install locations (apt/sdkman/etc).
JAVA_HOME := $(shell \
	if [ -n "$$JAVA_HOME" ] && "$$JAVA_HOME/bin/java" -version 2>&1 | grep -q '"21'; then \
		echo "$$JAVA_HOME"; \
	elif [ "$$(uname -s)" = "Darwin" ]; then \
		/usr/libexec/java_home -v 21 2>/dev/null; \
	else \
		for d in /usr/lib/jvm/*21* "$$HOME"/.sdkman/candidates/java/*21*; do \
			[ -x "$$d/bin/java" ] && { echo "$$d"; break; }; \
		done; \
	fi)
JAR        := java-api/target/eudi-api-0.1.0.jar

.PHONY: all build build-engine build-api run start stop restart status logs \
        demo ui test test-engine test-api clean help

all: help

help:
	@echo "EUDI demo — no Docker needed"
	@echo ""
	@echo "  make run      build (if needed) and start engine + api"
	@echo "  make ui       open the demo console at $(API_URL)/"
	@echo "  make demo     run the 6-scenario scripted demo"
	@echo "  make status   show what's running and health"
	@echo "  make logs     tail both service logs (ctrl-c to stop watching)"
	@echo "  make stop     stop both services"
	@echo "  make restart  stop then run"
	@echo "  make test     run the Rust + Java test suites"
	@echo "  make clean    stop services and remove build artifacts"
	@echo ""
	@echo "Optional: ANTHROPIC_API_KEY=sk-... make run   (real LLM proposer;"
	@echo "unset -> StubLlmResolver, identical guardrail pipeline)"
	@echo ""
	@echo "On Windows: plain cmd/PowerShell has no 'make' and no POSIX shell"
	@echo "for these recipes. Two working options instead:"
	@echo "  1. Docker Desktop  -> docker compose up --build   (works as-is)"
	@echo "  2. WSL2 (Ubuntu)   -> run this Makefile from the WSL shell;"
	@echo "     JDK 21 + cargo installed inside WSL, same commands as above."

# ---- build -------------------------------------------------------------
# Real file targets (not phony "if missing" checks) so a source-file edit is
# enough to trigger a rebuild — `if [ ! -f jar ]` alone would keep serving a
# stale build forever once the jar exists once.

ENGINE_BIN := rust-engine/target/release/engine
ENGINE_SRC := $(shell find rust-engine/src -type f 2>/dev/null) rust-engine/Cargo.toml rust-engine/Cargo.lock
JAVA_SRC   := $(shell find java-api/src -type f 2>/dev/null) java-api/pom.xml

build: build-engine build-api
build-engine: $(ENGINE_BIN)
build-api: $(JAR)

$(ENGINE_BIN): $(ENGINE_SRC)
	@cd rust-engine && cargo build --release

$(JAR): $(JAVA_SRC)
	@if [ -z "$(JAVA_HOME)" ]; then \
		echo "error: JDK 21 not found (Byte Buddy/Mockito reject JDK 24)."; \
		echo "Install one and re-run, e.g.: brew install --cask temurin@21"; \
		exit 1; \
	fi
	@cd java-api && JAVA_HOME=$(JAVA_HOME) ./mvnw -q -DskipTests package

# ---- run / stop ----------------------------------------------------------

run: start

start: $(RUN_DIR)
	@if [ -f $(ENGINE_PID) ] && kill -0 $$(cat $(ENGINE_PID)) 2>/dev/null; then \
		echo "engine already running (pid $$(cat $(ENGINE_PID)))"; \
	else \
		$(MAKE) $(ENGINE_BIN); \
		echo "starting engine on :8081 ..."; \
		CONFIG_DIR=rust-engine/config $(ENGINE_BIN) > $(ENGINE_LOG) 2>&1 & echo $$! > $(ENGINE_PID); \
	fi
	@echo -n "waiting for engine health"; \
	for i in $$(seq 1 30); do \
		curl -sf $(ENGINE_URL)/health >/dev/null 2>&1 && { echo " ok"; break; }; \
		echo -n "."; sleep 1; \
		if [ $$i -eq 30 ]; then echo " FAILED — see $(ENGINE_LOG)"; exit 1; fi; \
	done
	@if [ -f $(API_PID) ] && kill -0 $$(cat $(API_PID)) 2>/dev/null; then \
		echo "api already running (pid $$(cat $(API_PID)))"; \
	else \
		$(MAKE) $(JAR); \
		if [ -z "$(JAVA_HOME)" ]; then \
			echo "error: JDK 21 not found (Byte Buddy/Mockito reject JDK 24)."; \
			echo "Install one and re-run, e.g.: brew install --cask temurin@21"; \
			exit 1; \
		fi; \
		echo "starting api on :8080 ..."; \
		(ENGINE_URL=$(ENGINE_URL) ANTHROPIC_API_KEY=$(ANTHROPIC_API_KEY) \
			$(JAVA_HOME)/bin/java -jar $(JAR) > $(API_LOG) 2>&1 & echo $$! > $(API_PID)); \
	fi
	@echo -n "waiting for api health"; \
	for i in $$(seq 1 40); do \
		curl -sf $(API_URL)/ >/dev/null 2>&1 && { echo " ok"; break; }; \
		echo -n "."; sleep 1; \
		if [ $$i -eq 40 ]; then echo " FAILED — see $(API_LOG)"; exit 1; fi; \
	done
	@owner=$$(lsof -t -i :8080 -sTCP:LISTEN 2>/dev/null | head -1); \
	tracked=$$(cat $(API_PID) 2>/dev/null); \
	if [ -n "$$owner" ] && [ "$$owner" != "$$tracked" ]; then \
		echo "WARNING: :8080 is being served by pid $$owner, not the pid this"; \
		echo "  run tracked ($$tracked) — a leftover process from an earlier"; \
		echo "  session is masking your rebuild. Run 'make stop' (now cleans up"; \
		echo "  untracked listeners on 8080/8081 too) then 'make run' again."; \
	fi
	@echo ""
	@echo "Both services up:"
	@echo "  Demo console : $(API_URL)/"
	@echo "  Swagger (api): $(API_URL)/swagger-ui.html"
	@echo "  Swagger (eng): $(ENGINE_URL)/swagger-ui/"

stop:
	@if [ -f $(API_PID) ]; then \
		kill $$(cat $(API_PID)) 2>/dev/null && echo "api stopped" || echo "api not running"; \
		rm -f $(API_PID); \
	else echo "api not running"; fi
	@if [ -f $(ENGINE_PID) ]; then \
		kill $$(cat $(ENGINE_PID)) 2>/dev/null && echo "engine stopped" || echo "engine not running"; \
		rm -f $(ENGINE_PID); \
	else echo "engine not running"; fi
	@for p in $$(lsof -t -i :8080 -sTCP:LISTEN 2>/dev/null) $$(lsof -t -i :8081 -sTCP:LISTEN 2>/dev/null); do \
		echo "killing orphaned process on 8080/8081 not tracked by a PID file (pid $$p)"; kill $$p 2>/dev/null; \
	done

restart: stop run

status:
	@printf "%-8s" "engine"; \
	if [ -f $(ENGINE_PID) ] && kill -0 $$(cat $(ENGINE_PID)) 2>/dev/null; then \
		curl -sf $(ENGINE_URL)/health >/dev/null 2>&1 && echo "running, healthy (pid $$(cat $(ENGINE_PID)))" \
			|| echo "running, NOT healthy (pid $$(cat $(ENGINE_PID)))"; \
	else echo "stopped"; fi
	@printf "%-8s" "api"; \
	if [ -f $(API_PID) ] && kill -0 $$(cat $(API_PID)) 2>/dev/null; then \
		curl -sf $(API_URL)/ >/dev/null 2>&1 && echo "running, healthy (pid $$(cat $(API_PID)))" \
			|| echo "running, NOT healthy (pid $$(cat $(API_PID)))"; \
	else echo "stopped"; fi

logs:
	@tail -f $(ENGINE_LOG) $(API_LOG)

$(RUN_DIR):
	@mkdir -p $(RUN_DIR)

# ---- demo / ui -----------------------------------------------------------

demo: run
	@./demo/run_demo.sh

ui: run
	@echo "opening $(API_URL)/"
	@open $(API_URL)/ 2>/dev/null \
		|| xdg-open $(API_URL)/ 2>/dev/null \
		|| powershell.exe -c "Start-Process '$(API_URL)/'" 2>/dev/null \
		|| echo "open $(API_URL)/ manually"

# ---- tests -----------------------------------------------------------

test: test-engine test-api

test-engine:
	@cd rust-engine && cargo test

test-api:
	@if [ -z "$(JAVA_HOME)" ]; then \
		echo "error: JDK 21 not found. Install one, e.g.: brew install --cask temurin@21"; \
		exit 1; \
	fi
	@cd java-api && JAVA_HOME=$(JAVA_HOME) ./mvnw test

# ---- clean -----------------------------------------------------------

clean: stop
	@cd rust-engine && cargo clean
	@cd java-api && ./mvnw -q clean 2>/dev/null || true
	@rm -rf $(RUN_DIR)
