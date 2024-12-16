import os
import re
import sys
import json
import argparse
import logging
from litellm import completion


def setup_logging(verbose: bool) -> None:
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(
        level=level,
        format="%(asctime)s - %(levelname)s - %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )


def read_file_content(file_path: str) -> str:
    logging.debug(f"Reading file: {file_path}")
    with open(file_path, "r") as f:
        return f.read()


def parse_markdown_codeblock(text: str, language: str | None = None) -> str:
    pattern = r"```(\w*)\n([\s\S]*?)\n```"
    matches = re.finditer(pattern, text)
    valid_results = []
    for match in matches:
        block_language = match.group(1)
        parsed_content = match.group(2).strip()
        if language is None:
            return parsed_content
        if block_language == language:
            valid_results.append(parsed_content)
    if len(valid_results) == 0:
        raise ValueError("No matching markdown code blocks found")
    return valid_results[-1]


def validate_openapi_spec(spec_json: str) -> None:
    try:
        spec = json.loads(spec_json)
        required_fields = ["openapi", "info", "paths"]
        for field in required_fields:
            if field not in spec:
                raise ValueError(f"Missing required field: {field}")
        if not spec["paths"]:
            raise ValueError("No paths defined in specification")
        logging.debug("OpenAPI specification validation passed")
    except json.JSONDecodeError as e:
        raise ValueError(f"Invalid JSON format: {e}")


def get_relevant_files(server_path: str) -> list[str]:
    logging.debug(f"Scanning directory: {server_path}")
    handlers_dir = os.path.join(server_path, "src", "handlers")
    relevant_files = [
        os.path.join(server_path, "src", "server.rs"),
        *[
            os.path.join(handlers_dir, f)
            for f in os.listdir(handlers_dir)
            if f.endswith(".rs")
        ],
    ]
    logging.debug(f"Found {len(relevant_files)} relevant files")
    return relevant_files


def build_prompt(code_contents: list[tuple[str, str]]) -> str:
    logging.debug("Building prompt with %d code files", len(code_contents))
    code_sections = "\n\n".join(
        [
            f"File: {file_path}\n```rust\n{content}\n```"
            for file_path, content in code_contents
        ]
    )
    return f"""# Task
Generate an OpenAPI 3.0 specification for the Rust web server code below.
The specification should be in JSON format and include all endpoints, request parameters, and response schemas.

# Code
{code_sections}

# Format
Respond with only a markdown code block containing the OpenAPI specification in JSON format.
The specification should follow OpenAPI 3.0 standards and include:
- All endpoints and their HTTP methods
- Request parameters (path, query, body)
- Response schemas and examples
- Component schemas for reusable types
"""


def generate_openapi_spec(server_path: str) -> str:
    logging.info("Starting OpenAPI specification generation")
    files = get_relevant_files(server_path)
    code_contents = []
    for file_path in files:
        relative_path = os.path.relpath(file_path, server_path)
        content = read_file_content(file_path)
        code_contents.append((relative_path, content))

    logging.info("Sending request to Claude")
    prompt = build_prompt(code_contents)
    response = completion(
        model="claude-3-5-sonnet-20241022",
        messages=[
            {
                "role": "system",
                "content": "You are a helpful assistant that generates OpenAPI specifications from Rust code.",
            },
            {"role": "user", "content": prompt},
        ],
        temperature=0,
    )
    logging.info("Received response from Claude")
    try:
        spec_json = parse_markdown_codeblock(
            response.choices[0].message.content, "json"
        )
        validate_openapi_spec(spec_json)
        return spec_json
    except ValueError as e:
        logging.error(f"Failed to parse or validate OpenAPI spec: {e}")
        raise


def main():
    parser = argparse.ArgumentParser(
        description="Generate OpenAPI specification from Rust server code"
    )
    parser.add_argument("server_path", help="Path to the server directory")
    parser.add_argument("--output", "-o", help="Output file path (default: stdout)")
    parser.add_argument(
        "--verbose", "-v", action="store_true", help="Enable verbose logging"
    )
    args = parser.parse_args()

    setup_logging(args.verbose)

    if not os.path.exists(args.server_path):
        logging.error(f"Path does not exist: {args.server_path}")
        sys.exit(1)

    try:
        spec = generate_openapi_spec(args.server_path)
        if args.output:
            logging.info(f"Writing specification to {args.output}")
            with open(args.output, "w") as f:
                f.write(spec)
        else:
            print(spec)
        logging.info("OpenAPI specification generation completed successfully")
    except Exception as e:
        logging.error(f"Error generating OpenAPI spec: {e}", exc_info=True)
        sys.exit(1)


if __name__ == "__main__":
    main()
