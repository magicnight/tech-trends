use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use reqwest::Client;
use serde::Deserialize;

use super::Crawler;
use crate::models::{Source, Story};

const PATENTSVIEW_API: &str = "https://api.patentsview.org/patents/query";

pub struct PatentCrawler {
    client: Client,
    /// 搜索关键词
    keywords: Vec<String>,
}

impl PatentCrawler {
    pub fn new(keywords: Vec<String>) -> Self {
        Self {
            client: Client::new(),
            keywords,
        }
    }
}

#[derive(Debug, Deserialize)]
struct PatentResponse {
    patents: Option<Vec<PatentRecord>>,
}

#[derive(Debug, Deserialize)]
struct PatentRecord {
    patent_number: Option<String>,
    patent_title: Option<String>,
    patent_abstract: Option<String>,
    patent_date: Option<String>,
    #[serde(default)]
    inventors: Option<Vec<Inventor>>,
}

#[derive(Debug, Deserialize)]
struct Inventor {
    inventor_first_name: Option<String>,
    inventor_last_name: Option<String>,
}

#[async_trait]
impl Crawler for PatentCrawler {
    fn source_name(&self) -> &'static str {
        "patent"
    }

    async fn fetch(&self, limit: usize) -> Result<Vec<Story>> {
        let keyword_query = self.keywords.join(" OR ");
        let query = serde_json::json!({
            "q": {
                "_text_any": {
                    "patent_title": keyword_query
                }
            },
            "f": [
                "patent_number",
                "patent_title",
                "patent_abstract",
                "patent_date",
                "inventor_first_name",
                "inventor_last_name"
            ],
            "o": {
                "page": 1,
                "per_page": limit
            },
            "s": [{ "patent_date": "desc" }]
        });

        let resp: PatentResponse = self
            .client
            .post(PATENTSVIEW_API)
            .json(&query)
            .send()
            .await?
            .json()
            .await
            .context("Failed to query PatentsView")?;

        let patents = resp.patents.unwrap_or_default();
        let mut stories = Vec::new();

        for p in patents {
            let Some(number) = p.patent_number else {
                continue;
            };
            let title = p.patent_title.unwrap_or_default();
            let published_at = p
                .patent_date
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
                .unwrap_or_else(Utc::now);

            let author = p.inventors.and_then(|inv| {
                inv.first().map(|i| {
                    format!(
                        "{} {}",
                        i.inventor_first_name.as_deref().unwrap_or(""),
                        i.inventor_last_name.as_deref().unwrap_or("")
                    )
                    .trim()
                    .to_string()
                })
            });

            stories.push(Story {
                external_id: number.clone(),
                source: Source::Patent,
                title,
                url: Some(format!(
                    "https://patents.google.com/patent/US{number}"
                )),
                body: p.patent_abstract,
                author,
                published_at,
                score: None,
                metadata: None,
            });
        }

        Ok(stories)
    }
}
