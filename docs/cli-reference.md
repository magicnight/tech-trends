# CLI 命令参考

tech-trends 提供 10 个命令，覆盖数据同步、向量索引、分析预测、对话、话题管理的完整工作流。

## 全局选项

```bash
tech-trends [COMMAND]
tech-trends --help       # 查看帮助
tech-trends --version    # 查看版本
```

日志级别通过 `RUST_LOG` 环境变量控制：

```bash
RUST_LOG=debug tech-trends sync hn
RUST_LOG=tech_trends=trace tech-trends forecast "rust"
```

---

## sync — 数据同步

从指定数据源抓取最新内容，写入 SQLite 并自动向量化到 Qdrant。已存在的条目不会重复写入（增量同步）。HN 来源还会 BFS 一层展开评论。

```bash
tech-trends sync [SOURCE] [OPTIONS]
```

### 参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `SOURCE` | `all` | 数据来源：`hn`、`arxiv`、`patent`、`book`、`github`、`all` |
| `-l, --limit` | `30` | 每个来源抓取条数上限 |
| `--skip-index` | `false` | 跳过向量化，仅写入 SQLite |

### 示例

```bash
# 同步全部来源（含向量化）
tech-trends sync

# 只同步 Hacker News，最多 100 条
tech-trends sync hn --limit 100

# 同步 GitHub 热门项目
tech-trends sync github --limit 20

# 只同步 arXiv 论文
tech-trends sync arxiv

# 仅写入 SQLite，跳过向量化
tech-trends sync all --skip-index
```

### 数据来源详情

| 来源 | API | 默认配置 |
|------|-----|---------|
| `hn` | HN Firebase API | topstories, 并发 20, BFS 评论（每 story 最多 10 条） |
| `arxiv` | arXiv Export API | 分类: cs.AI, cs.LG, cs.SE |
| `patent` | PatentsView API | 关键词: artificial intelligence, machine learning |
| `book` | Google Books API | 出版社适配器: Manning, O'Reilly, Packt |
| `github` | GitHub Search API | 语言: rust/python/typescript, 主题: ml/ai/llm, 最近 7 天, ≥5 stars |

### 向量化行为

sync 写入 SQLite 后，自动对新内容执行：
1. Ollama embed 生成向量
2. FNV-1a 哈希生成 Qdrant point ID（HN 用原始数字 ID，评论 ID +10B 隔离）
3. Upsert 到 Qdrant（含 payload: title, source, url, body snippet, author）

如果 Qdrant 不可用，自动降级为仅写入 SQLite（显示警告）。

---

## reindex — 重建向量索引

从 SQLite 批量读取所有 stories 和 comments，重新 embed 并 upsert 到 Qdrant。用于更换 Embedding 模型或 Qdrant 数据丢失后恢复。

```bash
tech-trends reindex [OPTIONS]
```

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `-b, --batch-size` | `50` | 每批处理的 story 数量 |

```bash
# 默认批次重建
tech-trends reindex

# 大批次加速
tech-trends reindex --batch-size 200
```

---

## digest — 每日技术简报

从最近 24 小时入库的内容生成结构化技术简报。需要 LLM API。

```bash
tech-trends digest
```

输出内容：
- 3 条最值得关注的要点
- 按主题（非来源）分类的内容
- 每条信息标注原始来源（HN/arXiv/专利/书籍/GitHub）

---

## forecast — 趋势预测

对指定关键词进行跨源趋势预测，输出阶段判断和置信度。每次执行自动保存预测记录用于后续校准。

```bash
tech-trends forecast <KEYWORD>
```

### 输出字段

| 字段 | 说明 |
|------|------|
| 阶段 | `emerging` / `accelerating` / `maturing` / `declining` |
| 置信度 | `Low` / `Medium` / `High` |
| 置信度调整 | 基于历史校准的权重乘数（如有足够历史数据） |
| 窗口统计 | 30 天 / 90 天 / 180 天 / 总计的匹配数 |
| 来源分布 | 各数据源的匹配条数 |
| LLM 解读 | 趋势叙述性分析（需要 LLM API） |

### 示例

```bash
tech-trends forecast "rust"
tech-trends forecast "large language model"
tech-trends forecast "WebAssembly"
```

### 阶段判断规则

| 阶段 | 判据 |
|------|------|
| emerging | 仅社区讨论 |
| accelerating | 论文或专利开始出现 |
| maturing | 书籍 + 论文 + 专利均有信号 |
| declining | 所有指标下降 |

---

## backtest — 回测分析

用历史数据回测关键词趋势，验证预测的可证伪性。不需要 LLM。

```bash
tech-trends backtest <KEYWORD>
```

### 输出字段

| 字段 | 说明 |
|------|------|
| 窗口对比 | 30/90/180 天窗口的当前 vs 前期计数、delta、方向（↑↓→） |
| 领先信号源 | 最早出现增长信号的数据来源 |

```bash
tech-trends backtest "rust"
# 输出:
#   30天窗口: 当前=12, 前期=8, delta=4 ↑
#   90天窗口: 当前=35, 前期=28, delta=7 ↑
#   领先信号源: arxiv
```

---

## calibrate — 预测校准

对比历史预测记录与当前实际数据，评估预测准确性，输出置信度调整建议。

```bash
tech-trends calibrate <KEYWORD>
```

### 输出字段

| 字段 | 说明 |
|------|------|
| 历史预测数 | 该关键词累计保存的预测记录数 |
| 方向准确率 | 预测的趋势方向与实际方向的吻合率 |
| 置信度调整建议 | ↑ 建议提升 / ↓ 建议降低 / — 维持不变 |
| 历史对比 | 每条历史预测的详细对比明细 |

### 校准逻辑

- 准确率 ≥ 80%：建议提升置信度（权重 1.2x）
- 准确率 ≤ 40%：建议降低置信度（权重 0.8x）
- 中间范围：维持不变

```bash
tech-trends calibrate "rust"
# 输出:
#   历史预测数: 5
#   方向准确率: 75%
#   置信度调整建议: — 维持不变
```

---

## chat — RAG 对话

进入多轮 RAG 对话模式。需要 Qdrant 和 Ollama 运行中。

```bash
tech-trends chat
```

### 运行时依赖

- Qdrant 向量数据库（默认 `localhost:6334`）
- Ollama Embedding 服务（默认 `localhost:11434`）
- LLM API（默认 DeepSeek）

### 交互

```
你> Rust 在 AI 基础设施领域有什么新动向？
AI> 根据知识库检索到的文章...

你> exit
```

输入 `exit` 或 `quit` 退出。支持历史记录（上/下箭头）。

---

## topic — 话题管理

创建、查看和运行持续监控的话题分析流水线。

### topic create — 创建话题

```bash
tech-trends topic create <NAME> --keywords <KEYWORDS>
```

```bash
tech-trends topic create "Rust Agent" --keywords "rust,agent,autonomous,agentic"
```

### topic list — 列出话题

```bash
tech-trends topic list
# 输出:
#   [✓] Rust Agent — 关键词: rust, agent, autonomous, agentic
```

### topic run — 运行分析

```bash
# 运行指定话题
tech-trends topic run "Rust Agent"

# 运行全部已启用话题
tech-trends topic run
```

每个话题的分析流水线：forecast → backtest → 汇总报告 → 更新 `last_analyzed_at`。

---

## status — 数据库统计

显示数据库当前状态概览。

```bash
tech-trends status
# 输出:
#   Stories 总计: 256
#     hackernews: 120
#     arxiv: 60
#     github: 40
#     patent: 20
#     book: 16
#   HN 评论: 450
#   监控话题: 3
#   最近入库: 2026-03-30 08:15:00
#   最早数据: 2026-01-15 12:00:00
```
