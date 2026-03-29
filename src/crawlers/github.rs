use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use reqwest::Client;
use serde::Deserialize;

use super::Crawler;
use crate::models::{Source, Story};

const GITHUB_API: &str = "https://api.github.com";

/// GitHub Trending 爬虫 — 使用 Search API 获取近期热门仓库
pub struct GitHubCrawler {
    client: Client,
    /// 搜索的语言过滤（如 "rust", "python"），为空则不限
    languages: Vec<String>,
    /// 搜索的主题过滤（如 "machine-learning", "ai"）
    topics: Vec<String>,
}

impl GitHubCrawler {
    pub fn new(languages: Vec<String>, topics: Vec<String>) -> Self {
        Self {
            client: Client::builder()
                .user_agent("tech-trends/0.1")
                .build()
                .unwrap_or_else(|_| Client::new()),
            languages,
            topics,
        }
    }

    /// 构建 GitHub Search 查询字符串
    fn build_query(&self) -> String {
        let mut parts = Vec::new();

        // 最近 7 天创建的仓库
        let since = (Utc::now() - chrono::Duration::days(7))
            .format("%Y-%m-%d")
            .to_string();
        parts.push(format!("created:>{since}"));

        // 最少 5 stars 过滤噪声
        parts.push("stars:>=5".into());

        for lang in &self.languages {
            parts.push(format!("language:{lang}"));
        }

        for topic in &self.topics {
            parts.push(format!("topic:{topic}"));
        }

        parts.join(" ")
    }
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    items: Option<Vec<RepoItem>>,
}

#[derive(Debug, Deserialize)]
struct RepoItem {
    full_name: String,
    html_url: String,
    description: Option<String>,
    stargazers_count: i64,
    language: Option<String>,
    created_at: Option<String>,
    owner: Option<Owner>,
    topics: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct Owner {
    login: String,
}

#[async_trait]
impl Crawler for GitHubCrawler {
    fn source_name(&self) -> &'static str {
        "github"
    }

    async fn fetch(&self, limit: usize) -> Result<Vec<Story>> {
        let query = self.build_query();
        let per_page = limit.min(100); // GitHub API 单页最多 100

        let url = format!(
            "{GITHUB_API}/search/repositories?q={}&sort=stars&order=desc&per_page={per_page}",
            urlencoding(&query)
        );

        let resp: SearchResponse = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?
            .json()
            .await
            .context("Failed to query GitHub Search API")?;

        let items = resp.items.unwrap_or_default();
        let mut stories = Vec::new();

        for repo in items {
            let published_at = repo
                .created_at
                .as_deref()
                .and_then(|s| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ").ok())
                .map(|dt| dt.and_utc())
                .unwrap_or_else(Utc::now);

            let author = repo.owner.map(|o| o.login);

            let mut meta = serde_json::Map::new();
            if let Some(lang) = &repo.language {
                meta.insert("language".into(), serde_json::Value::String(lang.clone()));
            }
            if let Some(topics) = &repo.topics {
                meta.insert(
                    "topics".into(),
                    serde_json::Value::Array(
                        topics.iter().map(|t| serde_json::Value::String(t.clone())).collect(),
                    ),
                );
            }

            stories.push(Story {
                external_id: repo.full_name.clone(),
                source: Source::GitHub,
                title: repo.full_name,
                url: Some(repo.html_url),
                body: repo.description,
                author,
                published_at,
                score: Some(repo.stargazers_count),
                metadata: Some(serde_json::Value::Object(meta)),
            });
        }

        Ok(stories)
    }
}

/// 简单 URL 编码（空格 → +，保留基本字符）
fn urlencoding(s: &str) -> String {
    s.replace(' ', "+")
        .replace(':', "%3A")
        .replace('>', "%3E")
}
