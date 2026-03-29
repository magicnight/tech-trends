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
    /// 技术主题关键词（用于增强搜索精度，预留未来使用）
    #[allow(dead_code)]
    tech_topics: Vec<String>,
}

impl BookCrawler {
    pub fn new(publishers: Vec<String>) -> Self {
        Self {
            client: Client::new(),
            publishers,
            tech_topics: vec![
                "programming".into(),
                "software".into(),
                "machine learning".into(),
                "artificial intelligence".into(),
                "cloud computing".into(),
                "data science".into(),
                "rust".into(),
                "kubernetes".into(),
            ],
        }
    }

    /// 根据出版社路由到对应适配器
    async fn search_publisher(&self, publisher: &str, limit: usize) -> Result<Vec<Story>> {
        match publisher.to_lowercase().as_str() {
            "manning" => self.search_manning(limit).await,
            "o'reilly" | "oreilly" => self.search_oreilly(limit).await,
            "packt" => self.search_packt(limit).await,
            _ => self.search_google_books_generic(publisher, limit).await,
        }
    }

    /// Manning 专用适配器 — 通过 Google Books 带精确出版社名 + 技术主题过滤
    async fn search_manning(&self, limit: usize) -> Result<Vec<Story>> {
        // Manning 图书特征：标题常含 "in Action", "in Practice" 等
        let query = "inpublisher:Manning+subject:computers+(intitle:action+OR+intitle:practice+OR+intitle:programming)";
        self.fetch_google_books(query, "Manning", limit).await
    }

    /// O'Reilly 专用适配器 — 精确过滤 O'Reilly Media 出版物
    async fn search_oreilly(&self, limit: usize) -> Result<Vec<Story>> {
        // O'Reilly 图书特征：出版社名 "O'Reilly Media"
        let query = "inpublisher:\"O'Reilly+Media\"+subject:computers";
        self.fetch_google_books(query, "O'Reilly", limit).await
    }

    /// Packt 专用适配器 — 精确过滤 Packt Publishing
    async fn search_packt(&self, limit: usize) -> Result<Vec<Story>> {
        let query = "inpublisher:Packt+subject:computers";
        self.fetch_google_books(query, "Packt", limit).await
    }

    /// Google Books 通用 fallback
    async fn search_google_books_generic(
        &self,
        publisher: &str,
        limit: usize,
    ) -> Result<Vec<Story>> {
        let query = format!("inpublisher:{publisher}+subject:computers");
        self.fetch_google_books(&query, publisher, limit).await
    }

    /// 统一的 Google Books 查询执行
    async fn fetch_google_books(
        &self,
        query: &str,
        publisher_label: &str,
        limit: usize,
    ) -> Result<Vec<Story>> {
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
            .context(format!("Failed to query Google Books for {publisher_label}"))?;

        let items = resp.items.unwrap_or_default();
        let mut stories = Vec::new();

        for item in items {
            let info = item.volume_info;

            // 提取 ISBN（优先 ISBN-13）
            let isbn = info
                .industry_identifiers
                .and_then(|ids| {
                    ids.into_iter()
                        .find(|id| id.identifier_type == "ISBN_13")
                        .map(|id| id.identifier)
                })
                .unwrap_or_else(|| item.id.clone());

            let published_at = info
                .published_date
                .and_then(|d| {
                    chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                        .or_else(|_| chrono::NaiveDate::parse_from_str(&d, "%Y"))
                        .ok()
                })
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
                .unwrap_or_else(Utc::now);

            let author = info.authors.map(|a| a.join(", "));

            // 构建丰富的元数据
            let mut meta = serde_json::Map::new();
            meta.insert(
                "publisher".into(),
                serde_json::Value::String(publisher_label.into()),
            );
            if let Some(cats) = &info.categories {
                meta.insert(
                    "categories".into(),
                    serde_json::Value::Array(
                        cats.iter()
                            .map(|c| serde_json::Value::String(c.clone()))
                            .collect(),
                    ),
                );
            }
            if let Some(pages) = info.page_count {
                meta.insert("page_count".into(), serde_json::json!(pages));
            }
            if let Some(avg) = info.average_rating {
                meta.insert(
                    "rating".into(),
                    serde_json::Number::from_f64(avg)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null),
                );
            }

            stories.push(Story {
                external_id: isbn,
                source: Source::Book,
                title: info.title.unwrap_or_default(),
                url: info.info_link,
                body: info.description,
                author,
                published_at,
                score: None,
                metadata: Some(serde_json::Value::Object(meta)),
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
            match self.search_publisher(publisher, per_publisher).await {
                Ok(stories) => {
                    tracing::info!("{publisher}: 获取 {} 本书", stories.len());
                    all_stories.extend(stories);
                }
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
    categories: Option<Vec<String>>,
    #[serde(rename = "pageCount")]
    page_count: Option<i64>,
    #[serde(rename = "averageRating")]
    average_rating: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct IndustryId {
    #[serde(rename = "type")]
    identifier_type: String,
    identifier: String,
}
