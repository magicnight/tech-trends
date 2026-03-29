use anyhow::Result;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::models::{Comment, Source, Story};
use crate::vector::{EmbeddingClient, VectorStore};

/// HN 评论向量 ID 偏移量（避免与 story ID 冲突）
const COMMENT_ID_OFFSET: u64 = 10_000_000_000;

/// 将 Story 列表 embed 并 upsert 到 Qdrant
pub async fn index_stories(
    stories: &[Story],
    embedding: &EmbeddingClient,
    vector_store: &VectorStore,
) -> Result<usize> {
    let mut indexed = 0;
    for story in stories {
        let text = build_story_text(story);
        match embedding.embed(&text).await {
            Ok(vector) => {
                let point_id = story_to_point_id(story);
                let payload = story_to_payload(story);
                vector_store.upsert(point_id, vector, payload).await?;
                indexed += 1;
            }
            Err(e) => {
                tracing::warn!("Failed to embed story '{}': {e}", story.title);
            }
        }
    }
    Ok(indexed)
}

/// 将 HN 评论列表 embed 并 upsert 到 Qdrant
pub async fn index_comments(
    comments: &[Comment],
    embedding: &EmbeddingClient,
    vector_store: &VectorStore,
) -> Result<usize> {
    let mut indexed = 0;
    for comment in comments {
        if comment.text.trim().is_empty() {
            continue;
        }
        match embedding.embed(&comment.text).await {
            Ok(vector) => {
                let point_id = comment.external_id + COMMENT_ID_OFFSET;
                let mut payload = HashMap::new();
                payload.insert("source".into(), "hackernews_comment".into());
                payload.insert(
                    "story_id".into(),
                    serde_json::Value::String(comment.story_external_id.clone()),
                );
                payload.insert("body".into(), serde_json::Value::String(comment.text.clone()));
                if let Some(author) = &comment.author {
                    payload.insert("author".into(), serde_json::Value::String(author.clone()));
                }
                vector_store.upsert(point_id, vector, payload).await?;
                indexed += 1;
            }
            Err(e) => {
                tracing::warn!("Failed to embed comment {}: {e}", comment.external_id);
            }
        }
    }
    Ok(indexed)
}

/// Story → 用于 embedding 的文本
fn build_story_text(story: &Story) -> String {
    let mut text = story.title.clone();
    if let Some(body) = &story.body {
        let snippet: String = body.chars().take(1000).collect();
        text.push_str(". ");
        text.push_str(&snippet);
    }
    text
}

/// Story → Qdrant point ID（数字 ID 直接用，字符串 ID 用 FNV-1a 哈希）
fn story_to_point_id(story: &Story) -> u64 {
    match story.source {
        Source::HackerNews => story.external_id.parse::<u64>().unwrap_or_else(|_| fnv_hash(&story.external_id)),
        _ => fnv_hash(&story.external_id),
    }
}

/// FNV-1a 哈希：字符串 → u64
fn fnv_hash(s: &str) -> u64 {
    let mut hasher = fnv::FnvHasher::default();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Story → Qdrant payload（检索时用于展示）
fn story_to_payload(story: &Story) -> HashMap<String, serde_json::Value> {
    let mut payload = HashMap::new();
    payload.insert("title".into(), serde_json::Value::String(story.title.clone()));
    payload.insert("source".into(), serde_json::Value::String(story.source.as_str().into()));
    payload.insert(
        "external_id".into(),
        serde_json::Value::String(story.external_id.clone()),
    );
    if let Some(url) = &story.url {
        payload.insert("url".into(), serde_json::Value::String(url.clone()));
    }
    if let Some(body) = &story.body {
        let snippet: String = body.chars().take(500).collect();
        payload.insert("body".into(), serde_json::Value::String(snippet));
    }
    if let Some(author) = &story.author {
        payload.insert("author".into(), serde_json::Value::String(author.clone()));
    }
    payload.insert(
        "published_at".into(),
        serde_json::Value::String(story.published_at.to_rfc3339()),
    );
    payload
}
