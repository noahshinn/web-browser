searx:
	cd ./searxng-docker && docker compose up

dev-server:
	cd server && cargo run -- --port 8095
