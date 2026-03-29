# 架构设计

## 系统总览

```
┌──────────────────────────────────────────────────────────────┐
│                        tech-trends                            │
├──────────────┬──────────────┬────────────────────────────────┤
│  CLI (main)  │  Tauri (lib) │  MCP Server (planned)          │
├──────────────┴──────────────┴────────────────────────────────┤
│                      Services 层                              │
│  digest │ forecast │ backtest │ calibration │ chat │ topic    │
│                    indexer                                     │
├───────────────────────────┬──────────────────────────────────┤
│       Crawlers 层         │         AI 层                     │
│  HN │ arXiv │ Patent      │  Embedding (Ollama)               │
│  Book │ GitHub             │  LLM Client (DeepSeek)           │
├───────────────────────────┼──────────────────────────────────┤
│    SQLite (真相源)         │    Qdrant (向量索引)              │
│  5 tables                 │    FNV-1a ID 映射                 │
└───────────────────────────┴──────────────────────────────────┘
```

## 核心设计原则

### 1. CLI + Lib 分离

`main.rs` 只做命令分发和终端 I/O，所有业务逻辑在 `lib.rs` 导出的模块中。这使得 Tauri 桌面端可以直接调用同一套 `services::*` 函数，不需要重复实现。

### 2. SQLite 为真相源，Qdrant 为派生索引

所有原始数据存储在 SQLite 的 `stories` 表中。Qdrant 只存向量和 payload 摘要，作为检索加速层。

**好处**：如果 Qdrant 数据丢失或更换 Embedding 模型，可以用 `reindex` 命令从 SQLite 完整重建向量索引。

### 3. Embedding 和 LLM 分离

- **Embedding**：Ollama 本地推理（`nomic-embed-text`），零成本、零延迟、零隐私顾虑
- **LLM**：远程 API（DeepSeek），按需调用，可替换为任何 OpenAI 兼容接口

两者独立配置、独立运行，任意一侧故障不影响另一侧。

### 4. 统一时间轴

五个数据源格式完全不同，但都被规范化到 `stories` 表的 `published_at` 字段（RFC 3339 格式）。这是做跨源趋势分析的基础 — 没有统一时间轴，就没法做窗口对比。

### 5. 向量化与 sync 解耦

sync 写入 SQLite 后自动触发向量化，但如果 Qdrant 不可用，自动降级为仅写入 SQLite（显示警告）。`--skip-index` 参数可以显式跳过向量化。

## 数据模型

### stories 表（统一条目）

```sql
CREATE TABLE stories (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    external_id     TEXT    NOT NULL,     -- 平台内部 ID
    source          TEXT    NOT NULL,     -- hackernews/arxiv/patent/book/github
    title           TEXT    NOT NULL,
    url             TEXT,
    body            TEXT,                 -- 正文/摘要
    author          TEXT,
    published_at    TEXT    NOT NULL,     -- RFC 3339，统一时间轴
    score           INTEGER,             -- HN score / GitHub stars / 引用数
    metadata        TEXT,                -- JSON 额外数据
    created_at      TEXT    NOT NULL,     -- 入库时间
    UNIQUE(source, external_id)          -- 增量同步的关键约束
);
```

`UNIQUE(source, external_id)` 配合 `INSERT OR IGNORE` 实现增量同步：已存在的文章不会重复写入，不会污染趋势统计。

### comments 表（HN 评论）

```sql
CREATE TABLE comments (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    external_id         INTEGER NOT NULL UNIQUE,
    story_external_id   TEXT    NOT NULL,
    text                TEXT    NOT NULL,
    author              TEXT,
    published_at        TEXT    NOT NULL,
    created_at          TEXT    NOT NULL DEFAULT (datetime('now'))
);
```

### topics + topic_snapshots 表

话题系统采用快照模式：每次分析生成一条 `topic_snapshots` 记录，包含阶段、置信度、统计数据和 LLM 叙述。历史快照可用于追踪话题随时间的演变。

### calibration_records 表

```sql
CREATE TABLE calibration_records (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    keyword                 TEXT    NOT NULL,
    predicted_stage         TEXT    NOT NULL,
    predicted_confidence    TEXT    NOT NULL,
    count_30d               INTEGER NOT NULL,
    count_90d               INTEGER NOT NULL,
    count_180d              INTEGER NOT NULL,
    created_at              TEXT    NOT NULL DEFAULT (datetime('now'))
);
```

每次 `forecast` 执行时自动写入一条记录，`calibrate` 命令读取历史记录对比实际数据。

## 爬虫设计

### Crawler trait

```rust
#[async_trait]
pub trait Crawler: Send + Sync {
    fn source_name(&self) -> &'static str;
    async fn fetch(&self, limit: usize) -> Result<Vec<Story>>;
}
```

所有爬虫实现同一 trait，返回统一的 `Story` 列表。新增数据源只需实现此 trait。

### HN 爬虫：并发、增量与评论

- Firebase API 获取 topstories ID 列表
- Tokio Semaphore 控制并发（默认 20）
- BFS 一层展开评论（每 story 最多 10 条 kids）
- `INSERT OR IGNORE` 保证增量

### arXiv 爬虫：轻量 XML 解析

手写 XML tag 提取，不引入重量级 XML 库。arXiv Export API 格式十几年未变，tradeoff 可接受。

### GitHub 爬虫：Search API

- 查询最近 7 天创建、≥5 stars 的仓库
- 按语言（rust/python/typescript）和主题（ml/ai/llm）过滤
- 元数据包含 language、topics 列表
- 用 `full_name` 作为 external_id

### 专利爬虫：PatentsView

POST 查询美国专利局 PatentsView Search API，解析专利号、标题、摘要、发明人、日期。

### 书籍爬虫：适配器模式

```
BookCrawler::search_publisher(publisher)
    │
    ├─ "manning"  → 精确匹配 + 标题特征过滤（"in Action/Practice"）
    ├─ "o'reilly" → 精确匹配 "O'Reilly Media" 出版社
    ├─ "packt"    → 精确匹配 "Packt Publishing"
    └─ 其他       → Google Books API 通用 fallback
```

各适配器通过精细的 Google Books 查询参数实现差异化，元数据包含 categories、page_count、rating。新增出版社只需在 `search_publisher` 添加路由分支。

## 向量索引管道

### indexer 服务

`services/indexer.rs` 负责将 Stories 和 Comments embed 并 upsert 到 Qdrant：

```
Story/Comment
  → build_story_text（title + body snippet）
  → Ollama embed（生成 768 维向量）
  → story_to_point_id（FNV-1a 哈希映射）
  → VectorStore::upsert（写入 Qdrant + payload）
```

### ID 映射

| 数据类型 | 原始 ID | 向量 ID |
|---------|---------|---------|
| HN Story | 数字 ID | 原始数字 ID |
| HN Comment | 数字 ID | `id + 10_000_000_000`（ID 空间隔离） |
| arXiv 论文 | 字符串（`2401.12345`） | FNV-1a 哈希 → u64 |
| 专利 | 字符串（专利号） | FNV-1a 哈希 → u64 |
| 书籍 | ISBN / Google ID | FNV-1a 哈希 → u64 |
| GitHub 仓库 | `owner/repo` | FNV-1a 哈希 → u64 |

### RAG 流程

```
用户提问
  → Ollama embed（问题向量化）
  → Qdrant search（Top 5 最相似文章）
  → 构建上下文（标题 + 来源 + 摘要片段）
  → LLM 生成回答（要求标注引用来源编号）
```

## 趋势分析

### 为什么不用向量检索做趋势统计？

`forecast` 和 `backtest` 只用 SQLite 的 `LIKE` 关键词匹配，不走向量检索。原因：

- 趋势预测需要的是 **出现频率**，不是语义相似度
- `SELECT COUNT(*) WHERE title LIKE '%rust%'` 更直接、更可解释
- 向量检索留给 RAG 问答，那里才需要语义理解

### 置信度计算

`compute_confidence` 是纯规则函数，综合三个因素评分：

| 因素 | 权重 |
|------|------|
| 匹配总数 ≥ 50 | +2 |
| 匹配总数 ≥ 20 | +1 |
| 多源覆盖 ≥ 3 | +2 |
| 多源覆盖 ≥ 2 | +1 |
| 近期变化比 > 0.5 或 < 0.1 | +1 |

总分 0-2 → Low，3-4 → Medium，5+ → High

### 校准机制

每次 `forecast` 执行时，预测结果自动保存到 `calibration_records` 表。`calibrate` 命令读取历史记录，对比预测的趋势方向与实际数据变化：

- 方向准确率 ≥ 80% → 建议提升置信度（1.2x 权重）
- 方向准确率 ≤ 40% → 建议降低置信度（0.8x 权重）
- 中间范围 → 维持不变

校准权重在 `forecast` 输出中自动显示（当历史数据足够时）。

### 阶段判断

`compute_stage` 启发式规则：

1. 所有指标下降 → `declining`
2. 书籍 + 论文 + 专利均有 → `maturing`
3. 论文或专利出现 → `accelerating`
4. 仅社区讨论 → `emerging`

### 回测：领先信号源

`find_leading_signal` 比较各来源在近期 vs 远期窗口的增长率，找出最早出现增长的来源。例如：arXiv 论文密度 90 天前开始上升，而 HN 讨论最近才跟上 → arXiv 是领先信号。

## 未来扩展点

| 方向 | 设计考量 |
|------|---------|
| Tauri 桌面端 | `lib.rs` 已导出所有模块，Tauri command 直接调用 |
| MCP Server | 将 services 层包装为 MCP tools，供 AI Agent 调用 |
| 新数据源 | 实现 `Crawler` trait 即可 |
| 新出版社适配器 | `book.rs` 的 `search_publisher` 添加路由分支 |
| ratatui TUI | 复用 services 层，只做终端 UI 渲染 |
