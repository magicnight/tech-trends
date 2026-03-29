# CLI 命令参考

tect-brain 提供 6 个主命令，覆盖数据同步、分析、对话、话题管理的完整工作流。

## 全局选项

```bash
tect-brain [COMMAND]
tect-brain --help       # 查看帮助
tect-brain --version    # 查看版本
```

日志级别通过 `RUST_LOG` 环境变量控制：

```bash
RUST_LOG=debug tect-brain sync hn
RUST_LOG=tect_brain=trace tect-brain forecast "rust"
```

---

## sync — 数据同步

从指定数据源抓取最新内容并写入本地数据库。已存在的条目不会重复写入（增量同步）。

```bash
tect-brain sync [SOURCE] [OPTIONS]
```

### 参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `SOURCE` | `all` | 数据来源：`hn`、`arxiv`、`patent`、`book`、`all` |
| `-l, --limit` | `30` | 每个来源抓取条数上限 |

### 示例

```bash
# 同步全部来源，每个来源最多 30 条
tect-brain sync

# 只同步 Hacker News，最多 100 条
tect-brain sync hn --limit 100

# 只同步 arXiv 论文
tect-brain sync arxiv

# 同步专利数据
tect-brain sync patent
```

### 数据来源详情

| 来源 | API | 默认配置 |
|------|-----|---------|
| `hn` | HN Firebase API | topstories, 并发 20 |
| `arxiv` | arXiv Export API | 分类: cs.AI, cs.LG, cs.SE |
| `patent` | PatentsView API | 关键词: artificial intelligence, machine learning |
| `book` | Google Books API | 出版社: Manning, O'Reilly, Packt |

---

## digest — 每日技术简报

从最近 24 小时入库的内容生成结构化技术简报。需要 LLM API。

```bash
tect-brain digest
```

输出内容：
- 3 条最值得关注的要点
- 按主题（非来源）分类的内容
- 每条信息标注原始来源（HN/arXiv/专利/书籍）

### 依赖

- 需要配置 `TECT_LLM_API_KEY`
- 需要数据库中有最近 24 小时的同步数据

---

## forecast — 趋势预测

对指定关键词进行跨源趋势预测，输出阶段判断和置信度。

```bash
tect-brain forecast <KEYWORD>
```

### 参数

| 参数 | 说明 |
|------|------|
| `KEYWORD` | 要预测的技术关键词（支持多词，用引号包裹） |

### 输出字段

| 字段 | 说明 |
|------|------|
| 阶段 | `emerging` / `accelerating` / `maturing` / `declining` |
| 置信度 | `Low` / `Medium` / `High` |
| 窗口统计 | 30 天 / 90 天 / 180 天 / 总计的匹配数 |
| 来源分布 | 各数据源的匹配条数 |
| LLM 解读 | 趋势叙述性分析（需要 LLM API） |

### 示例

```bash
tect-brain forecast "rust"
tect-brain forecast "large language model"
tect-brain forecast "WebAssembly"
```

### 阶段判断规则

| 阶段 | 判据 |
|------|------|
| emerging | 仅社区讨论 |
| accelerating | 论文或专利开始出现 |
| maturing | 书籍 + 论文 + 专利均有信号 |
| declining | 所有指标下降 |

### 置信度计算

综合三个因素：
1. 匹配总数（数据充分度）
2. 多源覆盖数量
3. 180 天窗口内近期 vs 远期的变化幅度

---

## backtest — 回测分析

用历史数据回测关键词趋势，验证预测的可证伪性。不需要 LLM。

```bash
tect-brain backtest <KEYWORD>
```

### 输出字段

| 字段 | 说明 |
|------|------|
| 窗口对比 | 30/90/180 天窗口的当前 vs 前期计数、delta、方向（↑↓→） |
| 领先信号源 | 最早出现增长信号的数据来源 |

### 示例

```bash
tect-brain backtest "rust"
# 输出:
#   30天窗口: 当前=12, 前期=8, delta=4 ↑
#   90天窗口: 当前=35, 前期=28, delta=7 ↑
#   180天窗口: 当前=60, 前期=45, delta=15 ↑
#   领先信号源: arxiv
```

---

## chat — RAG 对话

进入多轮 RAG 对话模式。需要 Qdrant 和 Ollama 运行中。

```bash
tect-brain chat
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
tect-brain topic create <NAME> --keywords <KEYWORDS>
```

| 参数 | 说明 |
|------|------|
| `NAME` | 话题名称 |
| `-k, --keywords` | 关键词列表，逗号分隔 |

```bash
tect-brain topic create "Rust Agent" --keywords "rust,agent,autonomous,agentic"
```

### topic list — 列出话题

```bash
tect-brain topic list
# 输出:
#   [✓] Rust Agent — 关键词: rust, agent, autonomous, agentic
#   [✗] Old Topic — 关键词: ...
```

### topic run — 运行分析

```bash
# 运行指定话题的分析
tect-brain topic run "Rust Agent"

# 运行全部已启用话题
tect-brain topic run
```

每个话题的分析流水线包含：
1. 对每个关键词执行 forecast
2. 对每个关键词执行 backtest
3. 汇总生成话题报告（Markdown 格式）
4. 更新话题的 `last_analyzed_at` 时间戳
