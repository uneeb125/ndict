use crate::config::LlmConfig;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

pub struct LlmCleaner {
    api_url: String,
    client: reqwest::Client,
    model: String,
    system_prompt: String,
}

impl LlmCleaner {
    pub fn new(config: &LlmConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to build reqwest client for LLM");

        let api_url = if config.api_url.ends_with('/') {
            config.api_url.trim_end_matches('/').to_string()
        } else {
            config.api_url.clone()
        };

        Self {
            api_url,
            client,
            model: config.model.clone(),
            system_prompt: config.system_prompt.clone(),
        }
    }

    pub async fn clean(&self, text: &str) -> anyhow::Result<String> {
        let url = format!("{}/v1/chat/completions", self.api_url);

        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: self.system_prompt.clone(),
                },
                Message {
                    role: "user".to_string(),
                    content: format!("Raw transcription: {}", text),
                },
            ],
            stream: false,
            temperature: Some(0.1),
            max_tokens: Some(256),
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
            }),
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send request to LLM API")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "LLM API returned error {}: {}",
                status.as_u16(),
                body
            ));
        }

        let raw_body = response.text().await.context("Failed to read LLM response body")?;
        let cc_response: ChatCompletionResponse = serde_json::from_str(&raw_body)
            .context("Failed to parse LLM API response")?;

        let content = cc_response
            .choices
            .first()
            .map(|c| c.message.content.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("LLM returned empty response"))?;

        parse_cleaned_text(&content)
    }
}

fn parse_cleaned_text(content: &str) -> anyhow::Result<String> {
    match serde_json::from_str::<Value>(content) {
        Ok(json) => {
            if let Some(text) = json.get("cleaned_text").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    return Ok(text.to_string());
                }
            }
            Err(anyhow::anyhow!(
                "JSON response missing 'cleaned_text' field. Got: {}",
                content
            ))
        }
        Err(_) => {
            Ok(content.to_string())
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

#[derive(Debug, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChoiceMessage {
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> LlmConfig {
        LlmConfig {
            enabled: true,
            api_url: "http://localhost:11434".to_string(),
            model: "qwen2.5:0.5b".to_string(),
            system_prompt: "Clean up this text. Output JSON with a \"cleaned_text\" field.".to_string(),
            timeout_seconds: 10,
        }
    }

    #[test]
    fn test_llm_cleaner_new() {
        let config = test_config();
        let cleaner = LlmCleaner::new(&config);
        assert_eq!(cleaner.api_url, "http://localhost:11434");
        assert_eq!(cleaner.model, "qwen2.5:0.5b");
        assert_eq!(cleaner.system_prompt, "Clean up this text. Output JSON with a \"cleaned_text\" field.");
    }

    #[test]
    fn test_llm_cleaner_new_trailing_slash() {
        let mut config = test_config();
        config.api_url = "http://localhost:11434/".to_string();
        let cleaner = LlmCleaner::new(&config);
        assert_eq!(cleaner.api_url, "http://localhost:11434");
    }

    #[test]
    fn test_llm_cleaner_new_no_trailing_slash() {
        let config = test_config();
        let cleaner = LlmCleaner::new(&config);
        assert_eq!(cleaner.api_url, "http://localhost:11434");
    }

    #[test]
    fn test_parse_cleaned_text_valid_json() {
        let content = r#"{"cleaned_text": "Hello world!"}"#;
        let result = parse_cleaned_text(content).unwrap();
        assert_eq!(result, "Hello world!");
    }

    #[test]
    fn test_parse_cleaned_text_missing_field() {
        let content = r#"{"something": "else"}"#;
        let result = parse_cleaned_text(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_cleaned_text_empty_field() {
        let content = r#"{"cleaned_text": ""}"#;
        let result = parse_cleaned_text(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_cleaned_text_invalid_json_falls_back() {
        let content = "plain text output";
        let result = parse_cleaned_text(content).unwrap();
        assert_eq!(result, "plain text output");
    }

    #[test]
    fn test_parse_cleaned_text_extra_fields() {
        let content = r#"{"cleaned_text": "test", "extra": 123}"#;
        let result = parse_cleaned_text(content).unwrap();
        assert_eq!(result, "test");
    }
}
