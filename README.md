# web-browser

## Requirements

You need to have the following installed:

- [Docker](https://docs.docker.com/engine/install/) and [Docker Compose](https://docs.docker.com/compose/install/) (to run the searxng instance)
- [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) and [Rust](https://www.rust-lang.org/tools/install) (the search engine is written in Rust)

## To run

```bash
make server
```

This will start the server and the searxng instance.

You can test the web search server by running the following command:

```bash
curl http://localhost:8095/v1/agent_search?q=what%20is%20sequence%20parallelism
```

This will return a JSON object with the search results.

## Development

You can run the server with the following command:

```bash
make dev-server
```

You can also run the server with the following steps:

First, clone the submodules:

```bash
git submodule update --init --recursive
```

Make sure that the following environment variables are set:

```bash
export SEARX_PORT=8096
```

Then, run the searxng-docker container:

```bash
docker compose -f searxng-docker/docker-compose.yaml up -d
```

A searxng instance will be running on port 8096.

You can test the searxng instance by navigating to `http://localhost:8096/` in your browser or by running the following command:

```bash
curl http://localhost:8096/search?q=what%20is%20sequence%20parallelism
```

Then, open a new shell and set the following environment variables:

```bash
export SEARX_HOST=localhost
export SEARX_PORT=8096
export ANTHROPIC_API_KEY=...
export WEB_SEARCH_SERVER_PORT=8095
```

Then, run the server:

```bash
cd server && cargo run -- --port ${WEB_SEARCH_SERVER_PORT:-8095}
```
