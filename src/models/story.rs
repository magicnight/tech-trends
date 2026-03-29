use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 数据来源类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    HackerNews,
    Arxiv,
    Patent,
    Book,
    GitHub,
}

impl Source {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HackerNews => "hackernews",
            Self::Arxiv => "arxiv",
            Self::Patent => "patent",
            Self::Book => "book",
            Self::GitHub => "github",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "hackernews" => Some(Self::HackerNews),
            "arxiv" => Some(Self::Arxiv),
            "patent" => Some(Self::Patent),
            "book" => Some(Self::Book),
            "github" => Some(Self::GitHub),
            _ => None,
        }
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 统一的内容条目 — 所有数据源规范化到此结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Story {
    /// 来源平台内部 ID（HN id / arXiv id / patent number / ISBN）
    pub external_id: String,
    /// 数据来源
    pub source: Source,
    /// 标题
    pub title: String,
    /// URL（原文链接）
    pub url: Option<String>,
    /// 正文 / 摘要
    pub body: Option<String>,
    /// 作者
    pub author: Option<String>,
    /// 发布时间（统一时间轴）
    pub published_at: DateTime<Utc>,
    /// HN score / 引用数等
    pub score: Option<i64>,
    /// 额外元数据（JSON）
    pub metadata: Option<serde_json::Value>,
}

/// HN 评论
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    /// HN comment id
    pub external_id: u64,
    /// 所属 story 的 external_id
    pub story_external_id: String,
    /// 评论文本
    pub text: String,
    /// 作者
    pub author: Option<String>,
    /// 发布时间
    pub published_at: DateTime<Utc>,
}
