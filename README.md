# web-browser

## To run

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
make searx
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
```

Then, run the server:

```bash
make dev-server
```

The web search server will be running on port 8095.

You can test the web search server by running the following command:

```bash
curl http://localhost:8095/v1/agent_search?q=what%20is%20sequence%20parallelism
```

This will return a JSON object with the search results.
