use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Semaphore;
use std::sync::Arc;

use super::Crawler;
use crate::models::{Source, Story};

const HN_API_BASE: &str = "https://hacker-news.firebaseio.com/v0";
const MAX_CONCURRENT: usize = 20;

pub struct HnCrawler {
    client: Client,
    semaphore: Arc<Semaphore>,
}

impl HnCrawler {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct HnItem {
    id: u64,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    by: Option<String>,
    #[serde(default)]
    score: Option<i64>,
    #[serde(default)]
    time: Option<i64>,
    #[serde(default)]
    #[allow(dead_code)]
    kids: Option<Vec<u64>>,
    #[serde(default, rename = "type")]
    #[allow(dead_code)]
    item_type: Option<String>,
}

#[async_trait]
impl Crawler for HnCrawler {
    fn source_name(&self) -> &'static str {
        "hackernews"
    }

    async fn fetch(&self, limit: usize) -> Result<Vec<Story>> {
        let ids: Vec<u64> = self
            .client
            .get(format!("{HN_API_BASE}/topstories.json"))
            .send()
            .await?
            .json()
            .await
            .context("Failed to fetch HN top stories")?;

        let ids = &ids[..limit.min(ids.len())];
        let mut handles = Vec::with_capacity(ids.len());

        for &id in ids {
            let client = self.client.clone();
            let sem = self.semaphore.clone();
            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let url = format!("{HN_API_BASE}/item/{id}.json");
                let resp = client.get(&url).send().await?.json::<HnItem>().await?;
                Ok::<_, anyhow::Error>(resp)
            }));
        }

        let mut stories = Vec::new();
        for handle in handles {
            match handle.await? {
                Ok(item) => {
                    if let Some(title) = &item.title {
                        let published_at = item
                            .time
                            .and_then(|t| DateTime::from_timestamp(t, 0))
                            .unwrap_or_else(Utc::now);

                        stories.push(Story {
                            external_id: item.id.to_string(),
                            source: Source::HackerNews,
                            title: title.clone(),
                            url: item.url.clone(),
                            body: item.text.clone(),
                            author: item.by.clone(),
                            published_at,
                            score: item.score,
                            metadata: None,
                        });
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch HN item: {e}");
                }
            }
        }

        Ok(stories)
    }
}
