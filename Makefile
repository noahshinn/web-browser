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

.PHONY: build-server
build-server: check-llm-keys
	@echo "Building server in release mode..."
	@cd server && cargo build --release

.PHONY: services
services: check-llm-keys update-submodules
	@echo "Starting services..."
	@docker compose up -d
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