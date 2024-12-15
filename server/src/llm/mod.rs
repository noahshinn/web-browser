use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
pub mod openai;
pub mod anthropic;
pub mod custom;
pub mod fireworks;
pub mod gemini;

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

#[derive(Debug, Clone, Copy)]
pub enum Provider {
    OpenAI,
    Anthropic,
    Google,
    Fireworks,
    Custom,
}

#[derive(Debug, Clone)]
pub enum Model {
    GPT4o,
    GPT4oMini,
    Claude35Sonnet,
    Gemini2Flash,
    Gemini15Flash,
    Gemini15Flash8B,
    Gemini15Pro,
    Llama32Instruct1B,
    Llama32Instruct3B,
    Llama31Instruct8B,
    Llama32Vision11B,
    Llama32Instruct70B,
    Llama32Instruct405B,
    Custom,
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Model::GPT4o => write!(f, "gpt-4o"),
            Model::GPT4oMini => write!(f, "gpt-4o-mini"),
            Model::Claude35Sonnet => write!(f, "claude-3-5-sonnet-latest"),
            Model::Gemini2Flash => write!(f, "gemini-2.0-flash-exp"),
            Model::Gemini15Flash => write!(f, "gemini-1.5-flash"),
            Model::Gemini15Flash8B => write!(f, "gemini-1.5-flash-8b"),
            Model::Gemini15Pro => write!(f, "gemini-1.5-pro"),
            Model::Llama32Instruct1B => write!(f, "llama-v3p2-1b-instruct"),
            Model::Llama32Instruct3B => write!(f, "llama-v3p2-3b-instruct"),
            Model::Llama31Instruct8B => write!(f, "llama-v3p1-8b-instruct"),
            Model::Llama32Vision11B => write!(f, "llama-v3p2-11b-vision-instruct"),
            Model::Llama32Instruct70B => write!(f, "llama-v3p2-70b-instruct"),
            Model::Llama32Instruct405B => write!(f, "llama-v3p2-405b-instruct"),
            Model::Custom => write!(f, "custom"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CompletionBuilder {
    model: Option<Model>,
    provider: Option<Provider>,
    messages: Vec<Message>,
    temperature: Option<f64>,
    max_completion_tokens: Option<i32>,
    server_endpoint: Option<String>,
    custom_server_endpoint: Option<String>,
    custom_model: Option<String>,
}

impl CompletionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn model(mut self, model: Model) -> Self {
        self.model = Some(model);
        self
    }

    pub fn provider(mut self, provider: Provider) -> Self {
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

    pub fn server_endpoint(mut self, endpoint: String) -> Self {
        self.server_endpoint = Some(endpoint);
        self
    }

    pub fn custom_server_endpoint(mut self, endpoint: String) -> Self {
        self.custom_server_endpoint = Some(endpoint);
        self
    }

    pub fn custom_model(mut self, model: String) -> Self {
        self.custom_model = Some(model);
        self
    }

    pub async fn build(self) -> Result<String, LLMError> {
        let model = match self.model {
            Some(m) => m,
            None => return Err(LLMError::RequestBuildingError("model is required".to_string())),
        };

        let provider = match self.provider {
            Some(p) => p,
            None => return Err(LLMError::RequestBuildingError("provider is required".to_string())),
        };

        let options = CompletionOptions {
            temperature: self.temperature.unwrap_or(0.0),
            max_completion_tokens: self.max_completion_tokens.unwrap_or(0),
            server_endpoint: self.server_endpoint.unwrap_or_default(),
            custom_server_endpoint: self.custom_server_endpoint,
            custom_model: self.custom_model,
        };

        match provider {
            Provider::OpenAI => openai::completion_openai(model, &self.messages, Some(&options)).await,
            Provider::Anthropic => anthropic::completion_anthropic(model, &self.messages, Some(&options)).await,
            Provider::Google => gemini::completion_gemini(model, &self.messages, Some(&options)).await,
            Provider::Fireworks => fireworks::completion_fireworks(model, &self.messages, Some(&options)).await,
            Provider::Custom => custom::completion_custom(model, &self.messages, Some(&options)).await,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompletionOptions {
    pub temperature: f64,
    pub max_completion_tokens: i32,
    pub server_endpoint: String,
    pub custom_server_endpoint: Option<String>,
    pub custom_model: Option<String>,
}

#[derive(Error, Debug)]
pub enum LLMError {
    #[error("LLM request building failed: {0}")]
    RequestBuildingError(String),
    #[error("LLM request failed: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("Failed to parse response: {0}")]
    ParseError(String),
    #[error("LLM response is empty")]
    EmptyResponse,
}
