use crate::llm::{CompletionBuilder, LLMError, Model, Provider};
use crate::prompts::{
    Prompt, GENERATE_PARALLEL_QUERIES_SYSTEM_PROMPT, GENERATE_SEQUENTIAL_QUERIES_SYSTEM_PROMPT,
    GENERATE_SINGLE_QUERY_SYSTEM_PROMPT,
};
use crate::utils::{parse_json_response, ParseJsonError};
use rocket::form::FromFormField;
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Clone, Deserialize, FromFormField)]
pub enum QueryStrategy {
    #[serde(rename = "verbatim")]
    Verbatim,
    #[serde(rename = "single_synthesize")]
    SingleSynthesize,
    #[serde(rename = "parallel_synthesize")]
    ParallelSynthesize,
    #[serde(rename = "sequential_synthesize")]
    SequentialSynthesize,
}

impl Default for QueryStrategy {
    fn default() -> Self {
        QueryStrategy::Verbatim
    }
}

#[derive(Error, Debug)]
pub enum QuerySynthesisError {
    #[error("LLM error: {0}")]
    LLMError(#[from] LLMError),
    #[error("JSON parsing error: {0}")]
    JsonParsingError(#[from] ParseJsonError),
}

#[derive(Deserialize)]
pub struct QueryResponse {
    pub reasoning: String,
    pub query: String,
}

#[derive(Deserialize)]
pub struct MultiQueryResponse {
    pub reasoning: String,
    pub queries: Vec<String>,
}

async fn generate_single_query(original_query: &str) -> Result<QueryResponse, QuerySynthesisError> {
    let prompt = Prompt::new(
        GENERATE_SINGLE_QUERY_SYSTEM_PROMPT.to_string(),
        original_query.to_string(),
    );
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt.build_messages())
        .temperature(0.0)
        .build()
        .await
        .map_err(QuerySynthesisError::LLMError)
    {
        Ok(completion) => completion,
        Err(e) => return Err(e),
    };
    let query: QueryResponse = match parse_json_response(&completion) {
        Ok(query) => query,
        Err(e) => return Err(QuerySynthesisError::JsonParsingError(e)),
    };
    Ok(query)
}

async fn generate_parallel_queries(
    original_query: &str,
) -> Result<MultiQueryResponse, QuerySynthesisError> {
    let prompt = Prompt::new(
        GENERATE_PARALLEL_QUERIES_SYSTEM_PROMPT.to_string(),
        original_query.to_string(),
    );
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt.build_messages())
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(QuerySynthesisError::LLMError(e)),
    };
    let queries: MultiQueryResponse = match parse_json_response(&completion) {
        Ok(queries) => queries,
        Err(e) => return Err(QuerySynthesisError::JsonParsingError(e)),
    };
    Ok(queries)
}

async fn generate_sequential_queries(
    original_query: &str,
) -> Result<MultiQueryResponse, QuerySynthesisError> {
    let prompt = Prompt::new(
        GENERATE_SEQUENTIAL_QUERIES_SYSTEM_PROMPT.to_string(),
        original_query.to_string(),
    );
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt.build_messages())
        .temperature(0.0)
        .build()
        .await
        .map_err(QuerySynthesisError::LLMError)
    {
        Ok(completion) => completion,
        Err(e) => return Err(e),
    };
    let queries: MultiQueryResponse = match parse_json_response(&completion) {
        Ok(queries) => queries,
        Err(e) => return Err(QuerySynthesisError::JsonParsingError(e)),
    };
    Ok(queries)
}

pub async fn synthesize_queries(
    original_query: &str,
    strategy: &QueryStrategy,
) -> Result<MultiQueryResponse, QuerySynthesisError> {
    match strategy {
        QueryStrategy::Verbatim => Ok(MultiQueryResponse {
            reasoning: "".to_string(),
            queries: vec![original_query.to_string()],
        }),
        QueryStrategy::SingleSynthesize => {
            let query = match generate_single_query(original_query).await {
                Ok(query) => query,
                Err(e) => return Err(e),
            };
            Ok(MultiQueryResponse {
                reasoning: query.reasoning,
                queries: vec![query.query],
            })
        }
        QueryStrategy::ParallelSynthesize => {
            let queries = match generate_parallel_queries(original_query).await {
                Ok(queries) => queries,
                Err(e) => return Err(e),
            };
            Ok(queries)
        }
        QueryStrategy::SequentialSynthesize => {
            let queries = match generate_sequential_queries(original_query).await {
                Ok(queries) => queries,
                Err(e) => return Err(e),
            };
            Ok(queries)
        }
    }
}
