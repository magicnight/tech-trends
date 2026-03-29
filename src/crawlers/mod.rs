pub mod arxiv;
pub mod book;
pub mod hn;
pub mod patent;

use anyhow::Result;
use async_trait::async_trait;

use crate::models::Story;

/// 所有爬虫实现此 trait
#[async_trait]
pub trait Crawler: Send + Sync {
    /// 数据源名称
    fn source_name(&self) -> &'static str;

    /// 抓取数据，返回统一的 Story 列表
    async fn fetch(&self, limit: usize) -> Result<Vec<Story>>;
}
