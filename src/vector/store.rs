use anyhow::{Context, Result};
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, PointStruct, SearchPointsBuilder,
    VectorParamsBuilder, UpsertPointsBuilder,
};
use qdrant_client::Qdrant;
use serde_json::Value;
use std::collections::HashMap;

pub struct VectorStore {
    client: Qdrant,
    collection: String,
}

impl VectorStore {
    pub async fn new(url: &str, collection: &str, dim: u64) -> Result<Self> {
        let client = Qdrant::from_url(url).build()?;

        // 如果集合不存在则创建
        let collections = client.list_collections().await?;
        let exists = collections
            .collections
            .iter()
            .any(|c| c.name == collection);

        if !exists {
            client
                .create_collection(
                    CreateCollectionBuilder::new(collection)
                        .vectors_config(VectorParamsBuilder::new(dim, Distance::Cosine)),
                )
                .await
                .context("Failed to create Qdrant collection")?;
        }

        Ok(Self {
            client,
            collection: collection.to_string(),
        })
    }

    /// 插入/更新向量点
    pub async fn upsert(
        &self,
        id: u64,
        vector: Vec<f32>,
        payload: HashMap<String, Value>,
    ) -> Result<()> {
        let point = PointStruct::new(
            id,
            vector,
            payload
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect::<HashMap<String, qdrant_client::qdrant::Value>>(),
        );

        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection, vec![point]).wait(true))
            .await
            .context("Failed to upsert vector")?;

        Ok(())
    }

    /// 语义检索最相似的 top_k 条结果
    pub async fn search(
        &self,
        query_vector: Vec<f32>,
        top_k: u64,
    ) -> Result<Vec<SearchResult>> {
        let results = self
            .client
            .search_points(
                SearchPointsBuilder::new(&self.collection, query_vector, top_k)
                    .with_payload(true),
            )
            .await
            .context("Failed to search vectors")?;

        Ok(results
            .result
            .into_iter()
            .map(|p| SearchResult {
                id: match p.id.unwrap().point_id_options.unwrap() {
                    qdrant_client::qdrant::point_id::PointIdOptions::Num(n) => n,
                    _ => 0,
                },
                score: p.score,
                payload: p
                    .payload
                    .into_iter()
                    .map(|(k, v)| (k, qdrant_value_to_json(v)))
                    .collect(),
            })
            .collect())
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: u64,
    pub score: f32,
    pub payload: HashMap<String, Value>,
}

fn qdrant_value_to_json(v: qdrant_client::qdrant::Value) -> Value {
    use qdrant_client::qdrant::value::Kind;
    match v.kind {
        Some(Kind::StringValue(s)) => Value::String(s),
        Some(Kind::IntegerValue(i)) => Value::Number(i.into()),
        Some(Kind::DoubleValue(d)) => {
            serde_json::Number::from_f64(d).map(Value::Number).unwrap_or(Value::Null)
        }
        Some(Kind::BoolValue(b)) => Value::Bool(b),
        _ => Value::Null,
    }
}
