use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// SQLite 数据库路径
    pub db_path: PathBuf,
    /// Qdrant 服务地址
    pub qdrant_url: String,
    /// Qdrant 集合名称
    pub qdrant_collection: String,
    /// Ollama 服务地址（用于本地 embedding）
    pub ollama_url: String,
    /// Ollama embedding 模型名
    pub embedding_model: String,
    /// LLM API 地址（OpenAI 兼容）
    pub llm_api_url: String,
    /// LLM API Key
    pub llm_api_key: String,
    /// LLM 模型名
    pub llm_model: String,
    /// Embedding 向量维度
    pub embedding_dim: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            db_path: PathBuf::from("tect-brain.db"),
            qdrant_url: "http://localhost:6334".into(),
            qdrant_collection: "tect_brain".into(),
            ollama_url: "http://localhost:11434".into(),
            embedding_model: "nomic-embed-text".into(),
            llm_api_url: "https://api.deepseek.com/v1".into(),
            llm_api_key: String::new(),
            llm_model: "deepseek-chat".into(),
            embedding_dim: 768,
        }
    }
}

impl Config {
    /// 从环境变量覆盖默认配置
    pub fn from_env() -> Self {
        let mut cfg = Self::default();

        if let Ok(v) = std::env::var("TECT_DB_PATH") {
            cfg.db_path = PathBuf::from(v);
        }
        if let Ok(v) = std::env::var("TECT_QDRANT_URL") {
            cfg.qdrant_url = v;
        }
        if let Ok(v) = std::env::var("TECT_OLLAMA_URL") {
            cfg.ollama_url = v;
        }
        if let Ok(v) = std::env::var("TECT_EMBEDDING_MODEL") {
            cfg.embedding_model = v;
        }
        if let Ok(v) = std::env::var("TECT_LLM_API_URL") {
            cfg.llm_api_url = v;
        }
        if let Ok(v) = std::env::var("TECT_LLM_API_KEY") {
            cfg.llm_api_key = v;
        }
        if let Ok(v) = std::env::var("TECT_LLM_MODEL") {
            cfg.llm_model = v;
        }

        cfg
    }

    pub fn data_dir(&self) -> &Path {
        self.db_path.parent().unwrap_or(Path::new("."))
    }
}
