use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// OpenAI 兼容的 LLM 客户端（DeepSeek / OpenAI / 任意兼容接口）
pub struct LlmClient {
    client: Client,
    api_url: String,
    api_key: String,
    model: String,
}

impl LlmClient {
    pub fn new(api_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            client: Client::new(),
            api_url: api_url.to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    /// 发送聊天请求，返回助手回复
    pub async fn chat(&self, messages: &[ChatMessage]) -> Result<String> {
        let resp: ChatResponse = self
            .client
            .post(format!("{}/chat/completions", self.api_url))
            .bearer_auth(&self.api_key)
            .json(&ChatRequest {
                model: &self.model,
                messages,
                temperature: 0.7,
            })
            .send()
            .await?
            .json()
            .await
            .context("Failed to parse LLM response")?;

        resp.choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .context("Empty LLM response")
    }

    /// 单次提问快捷方法
    pub async fn ask(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: system_prompt.into(),
            },
            ChatMessage {
                role: "user".into(),
                content: user_prompt.into(),
            },
        ];
        self.chat(&messages).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    temperature: f64,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: String,
}
