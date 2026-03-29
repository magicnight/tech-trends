use anyhow::Result;
use rusqlite::Connection;

/// 执行数据库迁移
pub fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS stories (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            external_id     TEXT    NOT NULL,
            source          TEXT    NOT NULL,
            title           TEXT    NOT NULL,
            url             TEXT,
            body            TEXT,
            author          TEXT,
            published_at    TEXT    NOT NULL,
            score           INTEGER,
            metadata        TEXT,
            created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
            UNIQUE(source, external_id)
        );

        CREATE INDEX IF NOT EXISTS idx_stories_source ON stories(source);
        CREATE INDEX IF NOT EXISTS idx_stories_published ON stories(published_at);
        CREATE INDEX IF NOT EXISTS idx_stories_title ON stories(title);

        CREATE TABLE IF NOT EXISTS comments (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            external_id         INTEGER NOT NULL,
            story_external_id   TEXT    NOT NULL,
            text                TEXT    NOT NULL,
            author              TEXT,
            published_at        TEXT    NOT NULL,
            created_at          TEXT    NOT NULL DEFAULT (datetime('now')),
            UNIQUE(external_id)
        );

        CREATE TABLE IF NOT EXISTS topics (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            name            TEXT    NOT NULL UNIQUE,
            keywords        TEXT    NOT NULL,
            enabled         INTEGER NOT NULL DEFAULT 1,
            created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
            last_analyzed_at TEXT
        );

        CREATE TABLE IF NOT EXISTS topic_snapshots (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            topic_id        INTEGER NOT NULL REFERENCES topics(id),
            analyzed_at     TEXT    NOT NULL DEFAULT (datetime('now')),
            stage           TEXT    NOT NULL,
            confidence      TEXT    NOT NULL,
            stats           TEXT    NOT NULL,
            narrative       TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_snapshots_topic ON topic_snapshots(topic_id);
        ",
    )?;
    Ok(())
}
