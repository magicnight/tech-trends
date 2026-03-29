use anyhow::Result;
use chrono::Utc;

use crate::db::Database;
use crate::llm::LlmClient;
use crate::models::Topic;
use crate::services::forecast;

/// 创建新话题
pub fn create_topic(db: &Database, name: &str, keywords: Vec<String>) -> Result<Topic> {
    let conn = db.conn();
    let kw_json = serde_json::to_string(&keywords)?;
    conn.execute(
        "INSERT INTO topics (name, keywords) VALUES (?1, ?2)",
        [name, &kw_json],
    )?;
    let id = conn.last_insert_rowid();
    Ok(Topic {
        id,
        name: name.to_string(),
        keywords,
        enabled: true,
        created_at: Utc::now(),
        last_analyzed_at: None,
    })
}

/// 列出所有话题
pub fn list_topics(db: &Database) -> Result<Vec<Topic>> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, name, keywords, enabled, created_at, last_analyzed_at FROM topics",
    )?;

    let topics = stmt
        .query_map([], |row| {
            let kw_str: String = row.get(2)?;
            let keywords: Vec<String> =
                serde_json::from_str(&kw_str).unwrap_or_default();
            Ok(Topic {
                id: row.get(0)?,
                name: row.get(1)?,
                keywords,
                enabled: row.get::<_, i64>(3)? != 0,
                created_at: row
                    .get::<_, String>(4)?
                    .parse()
                    .unwrap_or_else(|_| Utc::now()),
                last_analyzed_at: row
                    .get::<_, Option<String>>(5)?
                    .and_then(|s| s.parse().ok()),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(topics)
}

/// 执行话题分析流水线
pub async fn run_topic_pipeline(
    db: &Database,
    llm: &LlmClient,
    topic: &Topic,
) -> Result<String> {
    let mut results = Vec::new();

    for keyword in &topic.keywords {
        let forecast_result = forecast::forecast(db, llm, keyword).await?;
        let backtest_result = crate::services::backtest::backtest(db, keyword)?;

        results.push(format!(
            "### {keyword}\n\
             - 阶段: {}\n\
             - 置信度: {}\n\
             - 30天/90天/180天: {}/{}/{}\n\
             - 回测方向 (30d): {}\n\
             - 领先信号源: {}\n\
             {}",
            forecast_result.stage,
            forecast_result.confidence,
            forecast_result.windows.days_30,
            forecast_result.windows.days_90,
            forecast_result.windows.days_180,
            backtest_result
                .windows
                .first()
                .map(|w| w.direction.to_string())
                .unwrap_or_else(|| "-".into()),
            backtest_result
                .leading_signal
                .map(|s| s.to_string())
                .unwrap_or_else(|| "无".into()),
            forecast_result
                .narrative
                .as_deref()
                .unwrap_or("（无 LLM 解读）"),
        ));
    }

    // 更新最近分析时间
    let conn = db.conn();
    conn.execute(
        "UPDATE topics SET last_analyzed_at = datetime('now') WHERE id = ?1",
        [topic.id],
    )?;

    let report = format!("# 话题报告: {}\n\n{}", topic.name, results.join("\n\n"));
    Ok(report)
}
