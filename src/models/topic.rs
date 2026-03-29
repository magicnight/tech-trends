use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 持续监控的话题
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub id: i64,
    /// 话题名称
    pub name: String,
    /// 搜索关键词列表
    pub keywords: Vec<String>,
    /// 是否启用
    pub enabled: bool,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最近一次分析时间
    pub last_analyzed_at: Option<DateTime<Utc>>,
}

/// 话题分析快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicSnapshot {
    pub topic_id: i64,
    pub analyzed_at: DateTime<Utc>,
    /// 趋势阶段
    pub stage: TrendStage,
    /// 置信度
    pub confidence: Confidence,
    /// 各时间窗口的统计摘要（JSON）
    pub stats: serde_json::Value,
    /// LLM 生成的叙述性解读
    pub narrative: Option<String>,
}

/// 技术生命周期阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrendStage {
    Emerging,
    Accelerating,
    Maturing,
    Declining,
}

impl std::fmt::Display for TrendStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Emerging => write!(f, "emerging"),
            Self::Accelerating => write!(f, "accelerating"),
            Self::Maturing => write!(f, "maturing"),
            Self::Declining => write!(f, "declining"),
        }
    }
}

/// 置信度分级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "Low"),
            Self::Medium => write!(f, "Medium"),
            Self::High => write!(f, "High"),
        }
    }
}
