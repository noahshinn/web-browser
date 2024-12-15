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
dev-server: update-submodules searx
	@echo "Starting server..."
	@cd server && cargo run

.PHONY: build-server
build-server:
	@echo "Building server in release mode..."
	@cd server && cargo build --release

.PHONY: server
server: update-submodules searx build-server
	@echo "Starting server in production mode..."
	@cd server && ./target/release/server
