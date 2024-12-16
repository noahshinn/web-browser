use crate::search::SearchResult;
use regex::Regex;
use serde::de::DeserializeOwned;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseMarkdownCodeBlockError {
    #[error("No matching markdown code blocks found in response: {0}")]
    NoMatchingMarkdownCodeBlocksFound(String),
    #[error("Failed to parse JSON: {0}")]
    ParseJsonError(#[from] serde_json::Error),
}

pub fn parse_markdown_code_block(
    content: &str,
    language: Option<&str>,
) -> Result<String, ParseMarkdownCodeBlockError> {
    let re = Regex::new(r"```(\w*)\n([\s\S]*?)\n```").unwrap();
    let mut valid_results = Vec::new();
    for cap in re.captures_iter(content) {
        let block_language = cap.get(1).map_or("", |m| m.as_str());
        let parsed_content = cap.get(2).map_or("", |m| m.as_str()).trim();

        if language.is_none() {
            return Ok(parsed_content.to_string());
        }
        if block_language == language.unwrap() {
            valid_results.push(parsed_content.to_string());
        }
    }
    if valid_results.is_empty() {
        return Err(
            ParseMarkdownCodeBlockError::NoMatchingMarkdownCodeBlocksFound(content.to_string()),
        );
    }
    Ok(valid_results.last().unwrap().to_string())
}

pub fn display_content_preview(content: &str) -> String {
    let preview = content
        .split_whitespace()
        .take(100)
        .collect::<Vec<_>>()
        .join(" ");
    format!("{}...", preview)
}

pub fn display_search_results_with_indices(results: &[SearchResult]) -> String {
    results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            format!(
                "[{}] Title: {} ({})\nContent preview: {}",
                i,
                r.title,
                r.url,
                display_content_preview(&r.content)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn parse_json_response<T: DeserializeOwned>(
    completion: &str,
) -> Result<T, ParseMarkdownCodeBlockError> {
    let json_string = parse_markdown_code_block(completion, Some("json"))?;
    let parsed = serde_json::from_str(&json_string)?;
    Ok(parsed)
}

pub fn enforce_n_sequential_newlines(text: &str, n: usize) -> String {
    let mut result = String::with_capacity(text.len());
    let mut newline_count = 0;
    for c in text.chars() {
        if c == '\n' {
            newline_count += 1;
            if newline_count <= n {
                result.push(c);
            }
        } else {
            newline_count = 0;
            result.push(c);
        }
    }
    result
}
