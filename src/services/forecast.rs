use anyhow::Result;

use crate::db::Database;
use crate::llm::LlmClient;
use crate::models::{Confidence, Source, TrendStage};

/// 趋势预测结果
#[derive(Debug)]
pub struct ForecastResult {
    pub keyword: String,
    pub stage: TrendStage,
    pub confidence: Confidence,
    pub windows: WindowStats,
    pub source_breakdown: Vec<(Source, i64)>,
    pub narrative: Option<String>,
}

/// 各时间窗口统计
#[derive(Debug)]
pub struct WindowStats {
    pub days_30: i64,
    pub days_90: i64,
    pub days_180: i64,
    pub total: i64,
}

/// 对关键词进行趋势预测
pub async fn forecast(
    db: &Database,
    llm: &LlmClient,
    keyword: &str,
) -> Result<ForecastResult> {
    let conn = db.conn();
    let pattern = format!("%{keyword}%");

    // 各时间窗口计数
    let count_window = |days: i64| -> Result<i64> {
        let sql = format!(
            "SELECT COUNT(*) FROM stories
             WHERE title LIKE ?1
             AND published_at >= datetime('now', '-{days} days')"
        );
        let count: i64 = conn.query_row(&sql, [&pattern], |r| r.get(0))?;
        Ok(count)
    };

    let days_30 = count_window(30)?;
    let days_90 = count_window(90)?;
    let days_180 = count_window(180)?;
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM stories WHERE title LIKE ?1",
        [&pattern],
        |r| r.get(0),
    )?;

    // 按来源细分
    let mut stmt = conn.prepare(
        "SELECT source, COUNT(*) FROM stories
         WHERE title LIKE ?1
         GROUP BY source",
    )?;
    let source_breakdown: Vec<(Source, i64)> = stmt
        .query_map([&pattern], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .filter_map(|r| r.ok())
        .filter_map(|(s, c)| Source::from_str(&s).map(|src| (src, c)))
        .collect();

    let source_count = source_breakdown.len();
    let confidence = compute_confidence(total, source_count, days_30, days_180);
    let stage = compute_stage(&source_breakdown, days_30, days_90, days_180);

    // LLM 叙述性解读
    let stats_text = format!(
        "关键词: {keyword}\n30天: {days_30}, 90天: {days_90}, 180天: {days_180}, 总计: {total}\n\
         来源分布: {source_breakdown:?}\n阶段: {stage}, 置信度: {confidence}"
    );
    let narrative = llm
        .ask(
            "你是技术趋势分析师。根据以下统计数据，用 2-3 段中文生成趋势解读。",
            &stats_text,
        )
        .await
        .ok();

    Ok(ForecastResult {
        keyword: keyword.to_string(),
        stage,
        confidence,
        windows: WindowStats {
            days_30,
            days_90,
            days_180,
            total,
        },
        source_breakdown,
        narrative,
    })
}

/// 置信度计算：综合匹配总数、多源覆盖、近期变化幅度
fn compute_confidence(total: i64, source_count: usize, recent: i64, older: i64) -> Confidence {
    let mut score = 0;

    // 数据充分度
    if total >= 50 {
        score += 2;
    } else if total >= 20 {
        score += 1;
    }

    // 多源覆盖
    if source_count >= 3 {
        score += 2;
    } else if source_count >= 2 {
        score += 1;
    }

    // 近期变化强度
    if older > 0 {
        let ratio = recent as f64 / older as f64;
        if ratio > 0.5 || ratio < 0.1 {
            score += 1; // 明显变化 = 更可信的趋势判断
        }
    }

    match score {
        0..=2 => Confidence::Low,
        3..=4 => Confidence::Medium,
        _ => Confidence::High,
    }
}

/// 阶段判断：启发式规则
fn compute_stage(
    sources: &[(Source, i64)],
    days_30: i64,
    days_90: i64,
    days_180: i64,
) -> TrendStage {
    let has = |s: Source| sources.iter().any(|(src, c)| *src == s && *c > 0);

    let _has_hn = has(Source::HackerNews);
    let has_arxiv = has(Source::Arxiv);
    let has_patent = has(Source::Patent);
    let has_book = has(Source::Book);

    // 所有指标下降
    if days_180 > 0 && days_30 == 0 && days_90 < days_180 / 3 {
        return TrendStage::Declining;
    }

    // 书籍大量出版 → maturing
    if has_book && has_arxiv && has_patent {
        return TrendStage::Maturing;
    }

    // 论文和专利出现 → accelerating
    if has_arxiv || has_patent {
        return TrendStage::Accelerating;
    }

    // 只有社区讨论 → emerging
    TrendStage::Emerging
}
