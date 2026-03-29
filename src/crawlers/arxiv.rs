use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use reqwest::Client;

use super::Crawler;
use crate::models::{Source, Story};

const ARXIV_API: &str = "http://export.arxiv.org/api/query";

pub struct ArxivCrawler {
    client: Client,
    /// 搜索的分类，如 "cs.AI", "cs.LG"
    categories: Vec<String>,
}

impl ArxivCrawler {
    pub fn new(categories: Vec<String>) -> Self {
        Self {
            client: Client::new(),
            categories,
        }
    }

    /// 轻量 XML 解析：提取 <entry> 块中的字段
    fn parse_entries(xml: &str) -> Vec<Story> {
        let mut stories = Vec::new();

        for entry in xml.split("<entry>").skip(1) {
            let title = Self::extract_tag(entry, "title")
                .map(|s| s.replace('\n', " ").trim().to_string());
            let summary = Self::extract_tag(entry, "summary")
                .map(|s| s.replace('\n', " ").trim().to_string());
            let id = Self::extract_tag(entry, "id");
            let published = Self::extract_tag(entry, "published");
            let author = Self::extract_tag(entry, "name");

            if let (Some(title), Some(id)) = (title, id) {
                let arxiv_id = id.trim_start_matches("http://arxiv.org/abs/").to_string();
                let published_at = published
                    .and_then(|p| {
                        NaiveDateTime::parse_from_str(p.trim(), "%Y-%m-%dT%H:%M:%SZ").ok()
                    })
                    .map(|dt| dt.and_utc())
                    .unwrap_or_else(Utc::now);

                stories.push(Story {
                    external_id: arxiv_id,
                    source: Source::Arxiv,
                    title,
                    url: Some(id),
                    body: summary,
                    author,
                    published_at,
                    score: None,
                    metadata: None,
                });
            }
        }

        stories
    }

    fn extract_tag(xml: &str, tag: &str) -> Option<String> {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        let start = xml.find(&open)? + open.len();
        let end = xml.find(&close)?;
        if start < end {
            Some(xml[start..end].to_string())
        } else {
            None
        }
    }
}

#[async_trait]
impl Crawler for ArxivCrawler {
    fn source_name(&self) -> &'static str {
        "arxiv"
    }

    async fn fetch(&self, limit: usize) -> Result<Vec<Story>> {
        let cat_query = self
            .categories
            .iter()
            .map(|c| format!("cat:{c}"))
            .collect::<Vec<_>>()
            .join("+OR+");

        let url = format!(
            "{ARXIV_API}?search_query={cat_query}&start=0&max_results={limit}&sortBy=submittedDate&sortOrder=descending"
        );

        let xml = self
            .client
            .get(&url)
            .send()
            .await?
            .text()
            .await
            .context("Failed to fetch arXiv feed")?;

        Ok(Self::parse_entries(&xml))
    }
}
