use anyhow::Result;

use crate::llm::{ChatMessage, LlmClient};
use crate::vector::{EmbeddingClient, VectorStore};

/// RAG 对话引擎
pub struct ChatEngine<'a> {
    embedding: &'a EmbeddingClient,
    vector_store: &'a VectorStore,
    llm: &'a LlmClient,
    history: Vec<ChatMessage>,
}

impl<'a> ChatEngine<'a> {
    pub fn new(
        embedding: &'a EmbeddingClient,
        vector_store: &'a VectorStore,
        llm: &'a LlmClient,
    ) -> Self {
        Self {
            embedding,
            vector_store,
            llm,
            history: vec![ChatMessage {
                role: "system".into(),
                content: "你是一个技术情报助手。基于检索到的技术文章回答问题。\
                          所有回答都要有据可查，标注信息来源。如果检索结果中没有相关信息，\
                          请明确说明而不是凭空编造。"
                    .into(),
            }],
        }
    }

    /// 处理一轮用户提问
    pub async fn ask(&mut self, question: &str) -> Result<String> {
        // 1. 向量检索相关文章
        let query_vec = self.embedding.embed(question).await?;
        let results = self.vector_store.search(query_vec, 5).await?;

        // 2. 构建上下文
        let mut context_parts = Vec::new();
        for (i, r) in results.iter().enumerate() {
            let title = r
                .payload
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("未知标题");
            let source = r
                .payload
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("未知来源");
            let body = r
                .payload
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let snippet: String = body.chars().take(500).collect();
            context_parts.push(format!(
                "[{i}] 【{source}】{title}\n{snippet}"
            ));
        }
        let context = context_parts.join("\n\n");

        // 3. 组合提问
        let augmented_question = format!(
            "以下是从知识库中检索到的相关文章：\n\n{context}\n\n用户问题: {question}\n\n\
             请基于以上检索结果回答，并标注引用来源编号。"
        );

        self.history.push(ChatMessage {
            role: "user".into(),
            content: augmented_question,
        });

        // 4. 调用 LLM
        let reply = self.llm.chat(&self.history).await?;

        self.history.push(ChatMessage {
            role: "assistant".into(),
            content: reply.clone(),
        });

        Ok(reply)
    }
}
