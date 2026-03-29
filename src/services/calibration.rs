use anyhow::Result;

use crate::db::Database;
use crate::models::{Confidence, TrendStage};

/// 校准记录 — 存储历史预测与实际结果
#[derive(Debug)]
pub struct CalibrationRecord {
    pub keyword: String,
    pub predicted_stage: TrendStage,
    pub predicted_confidence: Confidence,
    pub predicted_30d: i64,
    pub actual_30d: Option<i64>,
    pub accurate: Option<bool>,
}

/// 保存一条预测记录（forecast 时调用）
pub fn save_prediction(
    db: &Database,
    keyword: &str,
    stage: TrendStage,
    confidence: Confidence,
    count_30d: i64,
    count_90d: i64,
    count_180d: i64,
) -> Result<()> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO calibration_records
         (keyword, predicted_stage, predicted_confidence, count_30d, count_90d, count_180d)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            keyword,
            format!("{stage}"),
            format!("{confidence}"),
            count_30d,
            count_90d,
            count_180d,
        ],
    )?;
    Ok(())
}

/// 校准分析 — 对比历史预测与当前实际数据
pub fn calibrate(db: &Database, keyword: &str) -> Result<CalibrationReport> {
    let conn = db.conn();
    let pattern = format!("%{keyword}%");

    // 查询该关键词的历史预测记录
    let mut stmt = conn.prepare(
        "SELECT predicted_stage, predicted_confidence, count_30d, count_90d, count_180d, created_at
         FROM calibration_records
         WHERE keyword = ?1
         ORDER BY created_at DESC
         LIMIT 10",
    )?;

    let records: Vec<HistoricalPrediction> = stmt
        .query_map([keyword], |row| {
            Ok(HistoricalPrediction {
                stage: row.get(0)?,
                confidence: row.get(1)?,
                count_30d: row.get(2)?,
                count_90d: row.get(3)?,
                count_180d: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    if records.is_empty() {
        return Ok(CalibrationReport {
            keyword: keyword.to_string(),
            total_predictions: 0,
            accuracy_rate: None,
            direction_accuracy: None,
            confidence_adjustment: ConfidenceAdjustment::None,
            details: Vec::new(),
        });
    }

    // 当前实际数据
    let current_30d: i64 = conn.query_row(
        "SELECT COUNT(*) FROM stories WHERE title LIKE ?1 AND published_at >= datetime('now', '-30 days')",
        [&pattern],
        |r| r.get(0),
    )?;

    // 对比每条历史预测
    let mut correct_direction = 0;
    let mut total_comparable = 0;
    let mut details = Vec::new();

    for (i, record) in records.iter().enumerate() {
        if i == 0 {
            continue; // 最新的没有可比较的后续数据
        }
        // 比较前一条预测时的 30d count 与当前数据
        let prev = &records[i - 1];
        let predicted_direction = if prev.count_30d > record.count_30d {
            "up"
        } else if prev.count_30d < record.count_30d {
            "down"
        } else {
            "flat"
        };

        let actual_direction = if current_30d > record.count_30d {
            "up"
        } else if current_30d < record.count_30d {
            "down"
        } else {
            "flat"
        };

        let is_correct = predicted_direction == actual_direction;
        if is_correct {
            correct_direction += 1;
        }
        total_comparable += 1;

        details.push(CalibrationDetail {
            date: record.created_at.clone(),
            predicted_stage: record.stage.clone(),
            predicted_30d: record.count_30d,
            actual_30d: current_30d,
            direction_correct: is_correct,
        });
    }

    let direction_accuracy = if total_comparable > 0 {
        Some(correct_direction as f64 / total_comparable as f64)
    } else {
        None
    };

    // 置信度调整建议
    let confidence_adjustment = match direction_accuracy {
        Some(acc) if acc >= 0.8 => ConfidenceAdjustment::Increase,
        Some(acc) if acc <= 0.4 => ConfidenceAdjustment::Decrease,
        _ => ConfidenceAdjustment::None,
    };

    Ok(CalibrationReport {
        keyword: keyword.to_string(),
        total_predictions: records.len(),
        accuracy_rate: direction_accuracy,
        direction_accuracy,
        confidence_adjustment,
        details,
    })
}

/// 获取校准后的置信度权重乘数
pub fn get_calibrated_weight(db: &Database, keyword: &str) -> f64 {
    match calibrate(db, keyword) {
        Ok(report) => match report.confidence_adjustment {
            ConfidenceAdjustment::Increase => 1.2,
            ConfidenceAdjustment::Decrease => 0.8,
            ConfidenceAdjustment::None => 1.0,
        },
        Err(_) => 1.0,
    }
}

// --- Types ---

#[derive(Debug)]
pub struct CalibrationReport {
    pub keyword: String,
    pub total_predictions: usize,
    pub accuracy_rate: Option<f64>,
    pub direction_accuracy: Option<f64>,
    pub confidence_adjustment: ConfidenceAdjustment,
    pub details: Vec<CalibrationDetail>,
}

#[derive(Debug)]
pub struct CalibrationDetail {
    pub date: String,
    pub predicted_stage: String,
    pub predicted_30d: i64,
    pub actual_30d: i64,
    pub direction_correct: bool,
}

#[derive(Debug)]
pub enum ConfidenceAdjustment {
    Increase, // 历史准确率高，可提升置信度
    Decrease, // 历史准确率低，应降低置信度
    None,     // 数据不足或中等准确率
}

impl std::fmt::Display for ConfidenceAdjustment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Increase => write!(f, "↑ 建议提升"),
            Self::Decrease => write!(f, "↓ 建议降低"),
            Self::None => write!(f, "— 维持不变"),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct HistoricalPrediction {
    stage: String,
    confidence: String,
    count_30d: i64,
    count_90d: i64,
    count_180d: i64,
    created_at: String,
}
