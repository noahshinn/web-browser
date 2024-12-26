use crate::llm::{CompletionOptions, LLMError, Message, Model, Role};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::env;

const GEMINI_API_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";

#[derive(Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
}

#[derive(Serialize)]
struct GeminiSystemInstruction {
    parts: GeminiPart,
}

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "generationConfig")]
    generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiSystemInstruction>,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiResponseContent,
}

#[derive(Deserialize)]
struct GeminiResponseContent {
    parts: Vec<GeminiResponsePart>,
}

#[derive(Deserialize)]
struct GeminiResponsePart {
    text: String,
}

pub(crate) async fn completion_gemini(
    model: Model,
    messages: &[Message],
    options: Option<&CompletionOptions>,
) -> Result<String, LLMError> {
    let mut contents = Vec::new();
    let mut generation_config = None;
    let mut system_content = None;

    for msg in messages {
        match msg.role {
            Role::System => {
                generation_config = Some(GeminiGenerationConfig {
                    temperature: options
                        .and_then(|opt| (opt.temperature != 0.0).then_some(opt.temperature)),
                });
                system_content = Some(msg.content.clone());
            }
            Role::User | Role::Assistant => {
                let role = match msg.role {
                    Role::User => Some("user".to_string()),
                    Role::Assistant => Some("model".to_string()),
                    _ => None,
                };
                contents.push(GeminiContent {
                    parts: vec![GeminiPart {
                        text: msg.content.clone(),
                    }],
                    role,
                });
            }
        }
    }

    let system_content = system_content.map(|content| GeminiSystemInstruction {
        parts: GeminiPart { text: content },
    });

    let req_body = GeminiRequest {
        contents,
        generation_config,
        system_instruction: system_content,
    };

    let api_key = match env::var("GOOGLE_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            return Err(LLMError::RequestBuildingError(
                "GOOGLE_API_KEY environment variable not set".to_string(),
            ))
        }
    };

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let url = format!("{GEMINI_API_URL}/{model}:generateContent?key={api_key}");

    let client = reqwest::Client::new();
    let response = match client
        .post(url)
        .headers(headers)
        .json(&req_body)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => return Err(LLMError::RequestError(e)),
    };
    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read error response".to_string());
        return Err(LLMError::Other(
            format!(
                "Gemini API request failed with status {}: {}",
                status, error_text
            )
            .into(),
        ));
    }

    let response_body: GeminiResponse = match response.json().await {
        Ok(body) => body,
        Err(e) => return Err(LLMError::RequestError(e)),
    };

    response_body
        .candidates
        .first()
        .and_then(|candidate| candidate.content.parts.first())
        .map(|part| part.text.clone())
        .ok_or(LLMError::EmptyResponse)
}
