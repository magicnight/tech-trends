use anyhow::Result;
use chrono::{Duration, Utc};

use crate::db::Database;
use crate::llm::LlmClient;

/// 生成每日技术简报
pub async fn generate_digest(db: &Database, llm: &LlmClient) -> Result<String> {
    let since = Utc::now() - Duration::hours(24);
    let since_str = since.format("%Y-%m-%dT%H:%M:%S").to_string();

    // 按来源分组查询最近入库的内容
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT source, title, url, body, author, published_at
         FROM stories
         WHERE created_at >= ?1
         ORDER BY source, published_at DESC",
    )?;

    let rows = stmt.query_map([&since_str], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;

    let mut sections: Vec<String> = Vec::new();
    let mut current_source = String::new();
    let mut items = Vec::new();

    for row in rows {
        let (source, title, url, body, _author, _pub_at) = row?;
        if source != current_source {
            if !items.is_empty() {
                sections.push(format!("## {current_source}\n{}", items.join("\n")));
                items.clear();
            }
            current_source = source;
        }
        let snippet = body
            .as_deref()
            .map(|b| {
                let truncated: String = b.chars().take(200).collect();
                format!("  摘要: {truncated}")
            })
            .unwrap_or_default();
        let link = url.as_deref().unwrap_or("-");
        items.push(format!("- **{title}**\n  链接: {link}\n{snippet}"));
    }
    if !items.is_empty() {
        sections.push(format!("## {current_source}\n{}", items.join("\n")));
    }

    if sections.is_empty() {
        return Ok("过去 24 小时内没有新入库的内容。".into());
    }

    let raw_digest = sections.join("\n\n");

    // 用 LLM 生成结构化简报
    let prompt = format!(
        "你是一个技术情报分析师。请根据以下最近入库的技术内容，生成一份简洁的中文技术简报。\n\
         按主题（而非来源）重新分类，标注每条信息的原始来源（HN/arXiv/专利/书籍），\n\
         并在开头给出 3 条最值得关注的要点。\n\n{raw_digest}"
    );

    let summary = llm
        .ask("你是技术情报分析师，输出 Markdown 格式。", &prompt)
        .await?;

    Ok(summary)
}
