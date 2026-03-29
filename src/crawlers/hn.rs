use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Semaphore;

use super::Crawler;
use crate::models::{Comment, Source, Story};

const HN_API_BASE: &str = "https://hacker-news.firebaseio.com/v0";
const MAX_CONCURRENT: usize = 20;

pub struct HnCrawler {
    client: Client,
    semaphore: Arc<Semaphore>,
}

/// fetch 返回的结果：stories + comments
pub struct HnFetchResult {
    pub stories: Vec<Story>,
    pub comments: Vec<Comment>,
}

impl HnCrawler {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT)),
        }
    }

    /// 抓取 stories 并 BFS 一层展开评论
    pub async fn fetch_with_comments(&self, limit: usize) -> Result<HnFetchResult> {
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
        let mut all_comment_ids: Vec<(String, u64)> = Vec::new(); // (story_id, comment_id)

        for handle in handles {
            match handle.await? {
                Ok(item) => {
                    if let Some(title) = &item.title {
                        let published_at = item
                            .time
                            .and_then(|t| DateTime::from_timestamp(t, 0))
                            .unwrap_or_else(Utc::now);

                        let story_id = item.id.to_string();

                        // 收集一层评论 ID
                        if let Some(kids) = &item.kids {
                            for &kid_id in kids.iter().take(10) {
                                all_comment_ids.push((story_id.clone(), kid_id));
                            }
                        }

                        stories.push(Story {
                            external_id: story_id,
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

        // BFS 一层：并发抓取评论
        let mut comment_handles = Vec::with_capacity(all_comment_ids.len());
        for (story_id, comment_id) in all_comment_ids {
            let client = self.client.clone();
            let sem = self.semaphore.clone();
            comment_handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let url = format!("{HN_API_BASE}/item/{comment_id}.json");
                let item: HnItem = client.get(&url).send().await?.json().await?;
                Ok::<_, anyhow::Error>((story_id, item))
            }));
        }

        let mut comments = Vec::new();
        for handle in comment_handles {
            match handle.await? {
                Ok((story_id, item)) => {
                    if let Some(text) = &item.text {
                        if !text.trim().is_empty() {
                            let published_at = item
                                .time
                                .and_then(|t| DateTime::from_timestamp(t, 0))
                                .unwrap_or_else(Utc::now);

                            comments.push(Comment {
                                external_id: item.id,
                                story_external_id: story_id,
                                text: text.clone(),
                                author: item.by.clone(),
                                published_at,
                            });
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch HN comment: {e}");
                }
            }
        }

        Ok(HnFetchResult { stories, comments })
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
        let result = self.fetch_with_comments(limit).await?;
        Ok(result.stories)
    }
}
