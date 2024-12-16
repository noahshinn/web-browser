.PHONY: update-submodules
update-submodules:
	@echo "Updating submodules..."
	@git submodule update --init --recursive --remote

.PHONY: searx
searx: update-submodules
	@echo "Starting searxng server..."
	@docker compose -f searxng-docker/docker-compose.yaml up -d
	@echo "\033[1m🔎 Searxng server started at \033[4mhttp://localhost:$(or $(SEARX_PORT),8096)\033[0m"

.PHONY: dev-server
dev-server: check-anthropic-key update-submodules searx
	@echo "Starting server..."
	@cd server && cargo run

.PHONY: build-server
build-server: check-anthropic-key
	@echo "Building server in release mode..."
	@cd server && cargo build --release

.PHONY: server
server: check-anthropic-key update-submodules searx build-server
	@echo "Starting server in production mode..."
	@cd server && ./target/release/server

.PHONY: check-anthropic-key
check-anthropic-key:
	@if [ -z "$$ANTHROPIC_API_KEY" ]; then \
		echo "Error: ANTHROPIC_API_KEY environment variable is not set"; \
		echo "Please set it with: export ANTHROPIC_API_KEY=your_api_key"; \
		exit 1; \
	fi
