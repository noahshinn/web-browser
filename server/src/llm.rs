use crate::prompts::Prompt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

const DEFAULT_LLM_PROXY_HOST: &str = "localhost";
const DEFAULT_LLM_PROXY_PORT: &str = "8097";
const DEFAULT_MODEL_NAME: &str = "gpt-4o";
const DEFAULT_PROVIDER: &str = "openai";

fn llm_proxy_url() -> String {
    let host =
        std::env::var("LLM_PROXY_HOST").unwrap_or_else(|_| DEFAULT_LLM_PROXY_HOST.to_string());
    let port =
        std::env::var("LLM_PROXY_PORT").unwrap_or_else(|_| DEFAULT_LLM_PROXY_PORT.to_string());
    format!("http://{}:{}", host, port)
}

fn llm_proxy_api_key() -> String {
    std::env::var("LLM_PROXY_API_KEY").unwrap()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<LLMResponseChoice>,
    pub usage: LLMResponseUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponseChoice {
    pub index: i32,
    pub message: LLMResponseMessage,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponseMessage {
    pub content: String,
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponseUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponseError {
    pub message: String,
    pub code: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Default)]
pub struct CompletionBuilder {
    model: Option<String>,
    provider: Option<String>,
    messages: Vec<Message>,
    temperature: Option<f64>,
    max_completion_tokens: Option<i32>,
}

impl CompletionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }

    pub fn provider(mut self, provider: String) -> Self {
        self.provider = Some(provider);
        self
    }

    pub fn messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
    }

    pub fn temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn max_completion_tokens(mut self, tokens: i32) -> Self {
        self.max_completion_tokens = Some(tokens);
        self
    }

    pub async fn build(self) -> Result<String, LLMError> {
        let client = Client::new();
        let messages: Vec<serde_json::Value> = self
            .messages
            .into_iter()
            .map(|msg| {
                json!({
                    "role": msg.role,
                    "content": msg.content
                })
            })
            .collect();
        let response = match client
            .post(format!("{}/v1/chat/completions", llm_proxy_url()))
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", llm_proxy_api_key()))
            .json(&json!({
                "model": self.model.unwrap_or(DEFAULT_MODEL_NAME.to_string()),
                "custom_llm_provider": self.provider.unwrap_or(DEFAULT_PROVIDER.to_string()),
                "messages": messages,
                "temperature": self.temperature.unwrap_or(0.0),
                "max_tokens": self.max_completion_tokens.unwrap_or(1000)
            }))
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => return Err(LLMError::RequestError(e)),
        };
        if !response.status().is_success() {
            let status = response.status();
            let error = match response.json::<LLMResponseError>().await {
                Ok(error) => error,
                Err(e) => return Err(LLMError::RequestError(e)),
            };
            return Err(LLMError::Other(format!(
                "HTTP error status {}: {}",
                status, error.message
            )));
        }
        let response_json = match response.json::<LLMResponse>().await {
            Ok(response_json) => response_json,
            Err(e) => return Err(LLMError::RequestError(e)),
        };
        if response_json.choices.is_empty() {
            return Err(LLMError::EmptyResponse);
        }
        Ok(response_json.choices[0].message.content.clone())
    }
}

#[derive(Error, Debug)]
pub enum LLMError {
    #[error("LLM request building failed: {0}")]
    RequestBuildingError(String),
    #[error("LLM request failed: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("LLM response is empty")]
    EmptyResponse,
    #[error("Other error: {0}")]
    Other(String),
}

pub async fn default_completion(prompt: &Prompt) -> Result<String, LLMError> {
    let builder = CompletionBuilder::new()
        .model(DEFAULT_MODEL_NAME.to_string())
        .provider(DEFAULT_PROVIDER.to_string())
        .messages(prompt.clone().build_messages())
        .temperature(0.0);
    builder.build().await
}
