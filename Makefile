.PHONY: update-submodules
update-submodules:
	@echo "Updating submodules..."
	@git submodule update --init --recursive --remote

.PHONY: check-llm-keys
check-llm-keys:
	@for key in ANTHROPIC_API_KEY OPENAI_API_KEY GEMINI_API_KEY LLM_PROXY_API_KEY; do \
		if [ -z "$${!key}" ]; then \
			echo "Error: $$key environment variable is not set"; \
			echo "Please set it with: export $$key=your_api_key"; \
			exit 1; \
		fi \
	done
	@if [ ! -z "$(ENABLE_LLM_LOGGING)" ]; then \
		for key in LANGFUSE_PUBLIC_KEY LANGFUSE_SECRET_KEY; do \
			if [ -z "$${!key}" ]; then \
				echo "Error: $$key environment variable is required when ENABLE_LLM_LOGGING is set" && \
				echo "Please set it with: export $$key=your_key" && \
				echo "If you do not have a key yet, do the following:" && \
				echo "  1. Run \`docker compose up --build langfuse-server\` to start the Langfuse server"; \
				echo "  2. Navigate to $$(or $(LANGFUSE_HOST),http://localhost:8098)/auth/sign-in in your browser"; \
				echo "  3. Create a new API key (you may need to create an org and project if you have not already)"; \
				echo "  4. Set LANGFUSE_PUBLIC_KEY and LANGFUSE_SECRET_KEY as environment variables"; \
				echo "  5. Stop the Langfuse container (the existing data will be persisted)"; \
				echo "  6. Complete"; \
				exit 1; \
			fi \
		done \
	fi

.PHONY: build-server
build-server: check-llm-keys
	@echo "Building server in release mode..."
	@cd server && cargo build --release

.PHONY: services
services: check-llm-keys update-submodules
	@echo "Starting services..."
	@if [ ! -z "$(ENABLE_LLM_LOGGING)" ]; then \
		docker compose --profile llm-logging up -d --remove-orphans; \
		echo "\033[1m📈 Langfuse server started at \033[4m$${LANGFUSE_HOST:-http://localhost:8098}\033[0m"; \
	else \
		docker compose --profile default up -d --remove-orphans; \
	fi
	@echo "\033[1m🔎 Searxng server started at \033[4mhttp://localhost:$(or $(SEARX_PORT),8096)\033[0m"
	@echo "\033[1m🤖 LLM proxy started at \033[4mhttp://localhost:$(or $(LLM_PROXY_PORT),8097)\033[0m"

.PHONY: dev-server
dev-server: services
	@echo "Starting server..."
	@cd server && cargo run

.PHONY: server
server: services build-server
	@echo "Starting server in production mode..."
	@cd server && ./target/release/server