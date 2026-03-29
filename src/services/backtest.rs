use anyhow::Result;

use crate::db::Database;
use crate::models::Source;

/// 回测结果
#[derive(Debug)]
pub struct BacktestResult {
    pub keyword: String,
    pub windows: Vec<WindowComparison>,
    pub leading_signal: Option<Source>,
}

/// 单个时间窗口的对比
#[derive(Debug)]
pub struct WindowComparison {
    pub window_days: i64,
    pub current_count: i64,
    pub previous_count: i64,
    pub delta: i64,
    pub direction: Direction,
}

#[derive(Debug)]
pub enum Direction {
    Up,
    Down,
    Flat,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Up => write!(f, "↑"),
            Self::Down => write!(f, "↓"),
            Self::Flat => write!(f, "→"),
        }
    }
}

/// 对关键词进行回测分析
pub fn backtest(db: &Database, keyword: &str) -> Result<BacktestResult> {
    let conn = db.conn();
    let pattern = format!("%{keyword}%");

    let mut windows = Vec::new();
    for days in [30, 90, 180] {
        // 当前窗口
        let current_sql = format!(
            "SELECT COUNT(*) FROM stories
             WHERE title LIKE ?1
             AND published_at >= datetime('now', '-{days} days')"
        );
        let current: i64 = conn.query_row(&current_sql, [&pattern], |r| r.get(0))?;

        // 前一个同等长度窗口
        let previous_sql = format!(
            "SELECT COUNT(*) FROM stories
             WHERE title LIKE ?1
             AND published_at >= datetime('now', '-{0} days')
             AND published_at < datetime('now', '-{days} days')",
            days * 2
        );
        let previous: i64 = conn.query_row(&previous_sql, [&pattern], |r| r.get(0))?;

        let delta = current - previous;
        let direction = if delta > 0 {
            Direction::Up
        } else if delta < 0 {
            Direction::Down
        } else {
            Direction::Flat
        };

        windows.push(WindowComparison {
            window_days: days,
            current_count: current,
            previous_count: previous,
            delta,
            direction,
        });
    }

    // 谁先动了？找到最早出现增长信号的来源
    let leading_signal = find_leading_signal(conn, &pattern)?;

    Ok(BacktestResult {
        keyword: keyword.to_string(),
        windows,
        leading_signal,
    })
}

/// 找到最早出现增长信号的数据来源
fn find_leading_signal(
    conn: &rusqlite::Connection,
    pattern: &str,
) -> Result<Option<Source>> {
    let mut stmt = conn.prepare(
        "SELECT source,
                SUM(CASE WHEN published_at >= datetime('now', '-90 days') THEN 1 ELSE 0 END) as recent,
                SUM(CASE WHEN published_at < datetime('now', '-90 days')
                         AND published_at >= datetime('now', '-180 days') THEN 1 ELSE 0 END) as older
         FROM stories
         WHERE title LIKE ?1
         GROUP BY source",
    )?;

    let mut best: Option<(Source, f64)> = None;

    let rows = stmt.query_map([pattern], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    for row in rows {
        let (source_str, recent, older) = row?;
        if let Some(source) = Source::from_str(&source_str) {
            if older > 0 {
                let growth = (recent - older) as f64 / older as f64;
                if best.is_none() || growth > best.as_ref().unwrap().1 {
                    best = Some((source, growth));
                }
            } else if recent > 0 && best.is_none() {
                best = Some((source, f64::INFINITY));
            }
        }
    }

    Ok(best.map(|(s, _)| s))
}
