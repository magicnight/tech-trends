use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Ollama embedding 客户端
pub struct EmbeddingClient {
    client: Client,
    base_url: String,
    model: String,
}

impl EmbeddingClient {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            model: model.to_string(),
        }
    }

    /// 为单条文本生成 embedding
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let resp: EmbeddingResponse = self
            .client
            .post(format!("{}/api/embed", self.base_url))
            .json(&EmbeddingRequest {
                model: &self.model,
                input: text,
            })
            .send()
            .await?
            .json()
            .await
            .context("Failed to get embedding from Ollama")?;

        resp.embeddings
            .into_iter()
            .next()
            .context("Empty embedding response")
    }

    /// 批量生成 embedding
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }
}

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    input: &'a str,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    embeddings: Vec<Vec<f32>>,
}
