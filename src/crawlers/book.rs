use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

use super::Crawler;
use crate::models::{Source, Story};

const GOOGLE_BOOKS_API: &str = "https://www.googleapis.com/books/v1/volumes";

/// 书籍爬虫 — 适配器模式，支持多出版社策略
pub struct BookCrawler {
    client: Client,
    /// 出版社列表
    publishers: Vec<String>,
}

impl BookCrawler {
    pub fn new(publishers: Vec<String>) -> Self {
        Self {
            client: Client::new(),
            publishers,
        }
    }

    /// Google Books API fallback
    async fn search_google_books(
        &self,
        publisher: &str,
        limit: usize,
    ) -> Result<Vec<Story>> {
        let query = format!("inpublisher:{publisher}+subject:computers");
        let url = format!(
            "{GOOGLE_BOOKS_API}?q={query}&orderBy=newest&maxResults={limit}&langRestrict=en"
        );

        let resp: GoogleBooksResponse = self
            .client
            .get(&url)
            .send()
            .await?
            .json()
            .await
            .context("Failed to query Google Books")?;

        let items = resp.items.unwrap_or_default();
        let mut stories = Vec::new();

        for item in items {
            let info = item.volume_info;
            let isbn = info
                .industry_identifiers
                .and_then(|ids| {
                    ids.into_iter()
                        .find(|id| id.identifier_type == "ISBN_13")
                        .or_else(|| None)
                        .map(|id| id.identifier)
                })
                .unwrap_or_else(|| item.id);

            let published_at = info
                .published_date
                .and_then(|d| {
                    // 尝试多种日期格式
                    chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                        .or_else(|_| chrono::NaiveDate::parse_from_str(&d, "%Y"))
                        .ok()
                })
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
                .unwrap_or_else(Utc::now);

            let author = info.authors.map(|a| a.join(", "));

            stories.push(Story {
                external_id: isbn,
                source: Source::Book,
                title: info.title.unwrap_or_default(),
                url: info.info_link,
                body: info.description,
                author,
                published_at,
                score: None,
                metadata: Some(serde_json::json!({
                    "publisher": publisher,
                })),
            });
        }

        Ok(stories)
    }
}

#[async_trait]
impl Crawler for BookCrawler {
    fn source_name(&self) -> &'static str {
        "book"
    }

    async fn fetch(&self, limit: usize) -> Result<Vec<Story>> {
        let per_publisher = (limit / self.publishers.len().max(1)).max(5);
        let mut all_stories = Vec::new();

        for publisher in &self.publishers {
            match self.search_google_books(publisher, per_publisher).await {
                Ok(stories) => all_stories.extend(stories),
                Err(e) => tracing::warn!("Failed to fetch books from {publisher}: {e}"),
            }
        }

        Ok(all_stories)
    }
}

// --- Google Books API types ---

#[derive(Debug, Deserialize)]
struct GoogleBooksResponse {
    items: Option<Vec<GoogleBookItem>>,
}

#[derive(Debug, Deserialize)]
struct GoogleBookItem {
    id: String,
    #[serde(rename = "volumeInfo")]
    volume_info: VolumeInfo,
}

#[derive(Debug, Deserialize)]
struct VolumeInfo {
    title: Option<String>,
    authors: Option<Vec<String>>,
    description: Option<String>,
    #[serde(rename = "publishedDate")]
    published_date: Option<String>,
    #[serde(rename = "industryIdentifiers")]
    industry_identifiers: Option<Vec<IndustryId>>,
    #[serde(rename = "infoLink")]
    info_link: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IndustryId {
    #[serde(rename = "type")]
    identifier_type: String,
    identifier: String,
}
