# 配置说明

tect-brain 通过环境变量进行配置。所有变量均有合理的默认值，最少只需配置 LLM API Key 即可运行。

## 环境变量一览

| 变量名 | 默认值 | 说明 |
|--------|--------|------|
| `TECT_DB_PATH` | `tect-brain.db` | SQLite 数据库文件路径 |
| `TECT_QDRANT_URL` | `http://localhost:6334` | Qdrant 向量数据库地址 |
| `TECT_OLLAMA_URL` | `http://localhost:11434` | Ollama 服务地址 |
| `TECT_EMBEDDING_MODEL` | `nomic-embed-text` | Embedding 模型名称 |
| `TECT_LLM_API_URL` | `https://api.deepseek.com/v1` | LLM API 地址（OpenAI 兼容） |
| `TECT_LLM_API_KEY` | *(空)* | LLM API Key **（必须配置）** |
| `TECT_LLM_MODEL` | `deepseek-chat` | LLM 模型名称 |
| `RUST_LOG` | *(未设置)* | 日志级别，如 `debug`、`tect_brain=trace` |

## 最小配置

只需一个环境变量即可开始使用基本功能：

```bash
export TECT_LLM_API_KEY="sk-your-api-key"
```

> `sync` 和 `backtest` 命令不需要 LLM API Key。

## .env 文件

推荐在项目目录创建 `.env` 文件（已被 .gitignore 忽略）：

```bash
# .env
TECT_LLM_API_KEY=sk-your-deepseek-key
TECT_LLM_MODEL=deepseek-chat
TECT_DB_PATH=./data/tect-brain.db
```

> 注意：当前版本需要手动 `source .env` 或使用 `dotenv` 等工具加载。

## LLM 提供商切换

tect-brain 的 LLM 客户端兼容任何 OpenAI 格式的 API。切换提供商只需修改两个变量：

### DeepSeek（默认）

```bash
TECT_LLM_API_URL=https://api.deepseek.com/v1
TECT_LLM_MODEL=deepseek-chat
TECT_LLM_API_KEY=sk-xxx
```

### OpenAI

```bash
TECT_LLM_API_URL=https://api.openai.com/v1
TECT_LLM_MODEL=gpt-4o
TECT_LLM_API_KEY=sk-xxx
```

### 本地 Ollama（LLM + Embedding 全本地）

```bash
TECT_LLM_API_URL=http://localhost:11434/v1
TECT_LLM_MODEL=llama3
TECT_LLM_API_KEY=ollama  # Ollama 不校验 key，随便填
```

### Anthropic (Claude) via 兼容代理

```bash
TECT_LLM_API_URL=https://your-proxy.com/v1
TECT_LLM_MODEL=claude-sonnet-4-20250514
TECT_LLM_API_KEY=sk-ant-xxx
```

## Embedding 配置

默认使用 Ollama 本地推理 `nomic-embed-text`（768 维），零成本、零延迟。

如需更换模型：

```bash
TECT_EMBEDDING_MODEL=mxbai-embed-large    # 1024 维
```

> 更换 Embedding 模型后，需要重新对已有数据做向量化（从 SQLite 重建 Qdrant 索引）。

## 向量维度

`nomic-embed-text` 输出 768 维向量，这是 `Config` 中 `embedding_dim` 的默认值。如果使用不同维度的模型，需要：

1. 删除 Qdrant 中已有的集合
2. 修改代码中 `embedding_dim` 的默认值（当前版本不通过环境变量配置此项）

## Qdrant 配置

默认连接 `localhost:6334`（gRPC 端口）。Qdrant 的 HTTP API 端口是 `6333`，tect-brain 使用 gRPC 连接。

### Docker 启动

```bash
# 基础启动（数据不持久化）
docker run -p 6334:6334 -p 6333:6333 qdrant/qdrant

# 数据持久化
docker run -d \
  -p 6334:6334 -p 6333:6333 \
  -v $(pwd)/qdrant_data:/qdrant/storage \
  qdrant/qdrant
```

## 数据库路径

默认在当前工作目录创建 `tect-brain.db`。建议指定固定路径避免多处创建：

```bash
TECT_DB_PATH=$HOME/.local/share/tect-brain/tect-brain.db
```
