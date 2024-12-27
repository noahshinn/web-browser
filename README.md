# Web Browser

A tool that automates web search, traversal, and extraction of information on unstructured web pages.

## Requirements

You need to have the following installed:

- [Docker](https://docs.docker.com/engine/install/) and [Docker Compose](https://docs.docker.com/compose/install/) (to run the searxng instance)
- [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) and [Rust](https://www.rust-lang.org/tools/install) (the search engine is written in Rust)

## To run

```bash
make server
```

This will start the search server and the searxng instance.

You can test the server by running the following command:

```bash
curl -X POST http://localhost:8095/v1/agent_search \
  -H "Content-Type: application/json" \
  -d '{
    "query": "what is sequence parallelism",
    "search_strategy": "human"
  }'
```

This will return a JSON object with the search results.

## Options

### Search strategies

Several search strategies are supported. You can specify the strategy in the JSON body:

```bash
curl -X POST http://localhost:8095/v1/agent_search \
  -H "Content-Type: application/json" \
  -d '{
    "query": "what is sequence parallelism",
    "search_strategy": "parallel"
  }'
```

The following strategies are supported:

- `human`: (default) Searches the web like a human (one result at a time) by choosing the most relevant webpage to visit at each step and terminating when the query is comprehensively answered.
- `parallel`: (fast) Searches the web in parallel by visiting all of the results at once and aggregating the results at the end.
- `sequential`: (slow) Searches the web in sequential by visiting the results one at a time.
- `parallel_tree`: (hybrid) Builds a dependency tree of the results and auto-optimizes the traversal to process all of the results in parallel while respecting dependencies.

### Query strategies

You can specify the query strategy in the JSON body:

```bash
curl -X POST http://localhost:8095/v1/agent_search \
  -H "Content-Type: application/json" \
  -d '{
    "query": "what is sequence parallelism",
    "query_strategy": "single"
  }'
```

The following query strategies are supported:

- `verbatim`: (default) Uses the original query.
- `single`: (fast) Synthesizes a single query to search.
- `parallel`: (fast) Synthesizes one or more queries to search; visits the results in parallel.
- `sequential`: (slow) Synthesizes one or more queries to search; visits the results sequentially.

### Number of results to visit

You can specify the number of results to visit with the `max_results_to_visit` field in the JSON body (default is 10).

### Whitelisting and blacklisting base URLs

You can specify the whitelisted and blacklisted base URLs with the `whitelisted_base_urls` and `blacklisted_base_urls` fields in the JSON body:

- `whitelisted_base_urls`: Only the results from the whitelisted base URLs will be visited.
- `blacklisted_base_urls`: The results from the blacklisted base URLs will not be visited.

For example, to whitelist `github.com`, you can run the following command:

```bash
curl -X POST http://localhost:8095/v1/agent_search \
  -H "Content-Type: application/json" \
  -d '{
    "query": "what is sequence parallelism",
    "whitelisted_base_urls": ["github.com"]
  }'
```

### Result format

You can specify the result format with the `result_format` field in the JSON body. The following formats are supported:

- `answer`: (default) Formats the result as an answer.
- `research_summary`: Formats the result as a research summary.
- `faq_article`: Formats the result as a FAQ article.
- `news_article`: Formats the result as a news article.
- `webpage`: Formats the result as a webpage.
- `custom`: Formats the result as a custom format according to the custom format description.

For example, to format the result as a research summary, you can run the following command:

```bash
curl -X POST http://localhost:8095/v1/agent_search \
  -H "Content-Type: application/json" \
  -d '{
    "query": "what is sequence parallelism",
    "result_format": "research_summary"
  }'
```

To format the result as a custom format (such as a markdown table), you can run the following command:

```bash
curl -X POST http://localhost:8095/v1/agent_search \
  -H "Content-Type: application/json" \
  -d '{
    "query": "what is the founding date of each of the top 10 market cap companies in the world",
    "result_format": "custom",
    "custom_result_format_description": "Format the results as a markdown table with the following columns: Company Name, Founding Date"
  }'
```

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
curl http://localhost:8096/search?q=what+is+sequence+parallelism
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
