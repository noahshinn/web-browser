use regex::Regex;
use crate::search::{SearchResult, perform_search, SearchError};
use crate::llm::{CompletionBuilder, Message, Model, Provider, Role, LLMError};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use std::fmt::Display;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnalysisDocument {
    content: String,
    used_results: Vec<SearchResult>,
    discarded_results: Vec<SearchResult>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentSearchResult {
    analysis: AnalysisDocument,
    raw_results: Vec<SearchResult>,
}

#[derive(Error, Debug)]
pub struct SearchResultAnalysisError(LLMError);

impl Display for SearchResultAnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Search result analysis failed: {}", self.0)
    }
}

#[derive(Error, Debug)]
pub struct InsufficientFindingsCheckError(LLMError);

impl Display for InsufficientFindingsCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Insufficient findings check failed: {}", self.0)
    }
}

#[derive(Error, Debug)]
pub struct SelectNextResultError(LLMError);

impl Display for SelectNextResultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to select next result: {}", self.0)
    }
}

#[derive(Error, Debug)]
pub enum AgentSearchError {
    #[error("Search failed: {0}")]
    SearchError(#[from] SearchError),
    #[error("Analysis failed: {0}")]
    AnalysisError(#[from] SearchResultAnalysisError),
    #[error("Insufficient findings check failed: {0}")]
    InsufficientFindingsCheckError(#[from] InsufficientFindingsCheckError),
    #[error("Failed to select next result: {0}")]
    SelectNextResultError(#[from] SelectNextResultError),
}

#[derive(Deserialize, Debug, Clone)]
struct LLMDecision {
    keep_current: bool,
    new_analysis: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct NextResultToVisit {
    index: usize,
}

#[derive(Deserialize, Debug, Clone)]
struct SufficientFindingsCheck {
    sufficient: bool,
}

const WEB_SEARCH_CONTEXT: &str = r#"You are serving a verify specific task within a web search tool for a large language model.
The context is that you are looping over a set of web search results to build a "findings" document."#;

const WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT: &str = "USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT";

fn build_analyze_result_system_prompt() -> String {
    format!(
        r#"# Task
You will be given a search query and an in-progress analysis document that contains web search findings and a new search result to analyze.
The current web search result may not be the last result, so you should not assume that it is the last result and that you have to add irrelevant information to the analysis for the sake of being comprehensive.
Your task is to read the contents of the current web search result and extract the relevant information (if any) according to the query.
Examples of relevant information include:
- Content that is relevant to the query
- Links to pages that are citations to the information that you are including
- Links to pages that should be visited

## General context
{WEB_SEARCH_CONTEXT}

## Format
Respond with the new findings document so far (or `{WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT}` - this will be parsed). If you need to add more information to the findings document, do so.
If there is no new information to add, respond with `{WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT}`.
If you respond with `{WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT}`, you will move on to the next web search result to analyze and will keep the same findings document so far.
If there is new information to add, rewrite the findings document so far with the new information.
Make sure to not remove any information that is necessary.
It is okay to reformat the findings document to better fit the new information (the current findings document was written without the context of the new search result that you are analyzing).
This response will be used as the new findings document so far.
Remember, you will be reading the contents of many different web search results, so you do not have to force-fit the findings document to the query.
Most of the time, you will find that there is no new information to add to the findings document so far."#
    )
}

fn build_select_next_result_system_prompt() -> String {
    format!(
        r#"# Task
You will be given a search query, an in-progress findings document, a list of visited search results, and a list of unvisited search results.
Your task is to select the next unvisited search result to visit.

## General context
{WEB_SEARCH_CONTEXT}

## Format
Each unvisited search result will be labeled with an index.
You must respond with a JSON object in a markdown code block in the following format:
```json
{{
    "index": <the index of the next unvisited search result to analyze>
}}
```

For example:
```json
{{
    "index": 1 // assuming that the next unvisited search result that you want to visit is at index 1
}}
```
"#
    )
}

fn build_sufficient_findings_document_prompt() -> String {
    format!(
        r#"# Task
You will be given a search query, an in-progress findings document, a list of visited search results, and a list of unvisited search results.
Your task is to determine if the findings document is sufficient to answer the query in full.
If the findings document is not sufficient or if there is an unvisited search result that should be visited before making a decision, return false.
Otherwise, return true.

## General context
{WEB_SEARCH_CONTEXT}

## Format
Respond with a JSON object in a markdown code block in the following format:
```json
{{
    "sufficient": <true or false>
}}
```"#
    )
}

#[derive(Error, Debug)]
pub enum ParseMarkdownCodeBlockError {
    #[error("No matching markdown code blocks found in response: {0}")]
    NoMatchingMarkdownCodeBlocksFound(String),
    #[error("Failed to parse JSON: {0}")]
    ParseJsonError(#[from] serde_json::Error),
}

fn parse_markdown_code_block(content: &str, language: Option<&str>) -> Result<String, ParseMarkdownCodeBlockError> {
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
        return Err(ParseMarkdownCodeBlockError::NoMatchingMarkdownCodeBlocksFound(content.to_string()));
    }
    Ok(valid_results.last().unwrap().to_string())
}

fn display_content_preview(content: &str) -> String {
    let preview = content.split_whitespace().take(100).collect::<Vec<_>>().join(" ");
    format!("{}...", preview)
}

fn display_search_results_with_indices(results: &[SearchResult]) -> String {
    results
        .iter()
        .enumerate()
        .map(|(i, r)| format!("[{}] Title: {} ({})\nContent preview: {}", i, r.title, r.url, display_content_preview(&r.content)))
        .collect::<Vec<_>>()
        .join("\n\n")
}

async fn analyze_result(
    query: &str,
    current_analysis: &str,
    result: &SearchResult,
) -> Result<LLMDecision, Box<dyn std::error::Error>> {
    let prompt = vec![
        Message {
            role: Role::System,
            content: build_analyze_result_system_prompt(),
        },
        Message {
            role: Role::User,
            content: format!("# Query:\n{}\n\n# Search result:\n{}\n\n# Current findings document:\n{}", query, result, current_analysis),
        },
    ];
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt)
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(Box::new(e)),
    };

    if completion.contains(&WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT) {
        return Ok(LLMDecision {
            keep_current: true,
            new_analysis: None,
        });
    }
    Ok(LLMDecision {
        keep_current: false,
        new_analysis: Some(completion),
    })
}

async fn select_next_result(
    query: &str,
    current_analysis: &str,
    visited_results: &[SearchResult],
    unvisited_results: &[SearchResult],
) -> Result<usize, AgentSearchError> {
    let prompt = vec![
        Message {
            role: Role::System,
            content: build_select_next_result_system_prompt(),
        },
        Message {
            role: Role::User,
            content: format!("# Query:\n{}\n\n# Current analysis:\n{}\n\n# Visited results:\n{}\n\n# Unvisited results:\n{}", query, current_analysis, display_search_results_with_indices(visited_results), display_search_results_with_indices(unvisited_results)),
        },
    ];
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt)
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(AgentSearchError::SelectNextResultError(SelectNextResultError(e))),
    };

    let json_string = match parse_markdown_code_block(&completion, Some("json")) {
        Ok(json_string) => json_string,
        Err(e) => return Err(AgentSearchError::SelectNextResultError(SelectNextResultError(LLMError::ParseError(format!("Failed to parse JSON: {}", e))))),
    };
    let decision: NextResultToVisit = match serde_json::from_str(&json_string) {
        Ok(decision) => decision,
        Err(e) => return Err(AgentSearchError::SelectNextResultError(SelectNextResultError(LLMError::ParseError(format!("Failed to parse JSON: {}", e))))),
    };
    Ok(decision.index)
}

async fn check_sufficient_findings_document(
    query: &str,
    current_analysis: &str,
    used_results: &[SearchResult],
) -> Result<SufficientFindingsCheck, Box<dyn std::error::Error>> {
    let prompt = vec![
        Message {
            role: Role::System,
            content: build_sufficient_findings_document_prompt(),
        },
        Message {
            role: Role::User,
            content: format!("# Query:\n{}\n\n# Current analysis:\n{}\n\n# Used results:\n{}", query, current_analysis, display_search_results_with_indices(used_results)),
        },
    ];
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt)
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(Box::new(e)),
    };

    let json_string = match parse_markdown_code_block(&completion, Some("json")) {
        Ok(json_string) => json_string,
        Err(e) => return Err(Box::new(e)),
    };
    let decision: SufficientFindingsCheck = match serde_json::from_str(&json_string) {
        Ok(decision) => decision,
        Err(e) => return Err(Box::new(e)),
    };
    Ok(decision)
}

pub async fn perform_agent_search(query: &str, searx_host: &str, searx_port: &str) -> Result<AgentSearchResult, AgentSearchError> {
    let search_result = match perform_search(query, searx_host, searx_port).await {
        Ok(results) => results,
        Err(e) => return Err(AgentSearchError::SearchError(e)),
    };

    let mut analysis = AnalysisDocument {
        content: String::new(),
        used_results: Vec::new(),
        discarded_results: Vec::new(),
    };
    let mut unvisited_results = search_result.clone();

    while !unvisited_results.is_empty() {
        let next_index = match select_next_result(query, &analysis.content, &analysis.used_results, &unvisited_results).await {
            Ok(idx) => idx,
            Err(e) => return Err(e),
        };

        let result = unvisited_results.remove(next_index);
        match analyze_result(query, &analysis.content, &result).await {
            Ok(decision) => {
                if decision.keep_current {
                    analysis.discarded_results.push(result);
                } else if let Some(new_analysis) = decision.new_analysis {
                    analysis.content = new_analysis;
                    analysis.used_results.push(result);
                }
            }
            Err(e) => return Err(AgentSearchError::AnalysisError(SearchResultAnalysisError(LLMError::ParseError(format!("Failed to parse JSON: {}", e))))),
        }

        let sufficient = match check_sufficient_findings_document(query, &analysis.content, &analysis.used_results).await {
            Ok(decision) => decision.sufficient,
            Err(e) => return Err(AgentSearchError::InsufficientFindingsCheckError(InsufficientFindingsCheckError(LLMError::ParseError(format!("Failed to parse JSON: {}", e))))),
        };

        if sufficient {
            break;
        }
    }

    Ok(AgentSearchResult {
        analysis: analysis,
        raw_results: search_result,
    })
}