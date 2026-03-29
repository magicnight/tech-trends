# tech-trends

本地优先的 AI 驱动技术雷达 — 从 Hacker News 到 arXiv 论文，从专利数据到技术书籍，再到 GitHub 热门项目。

## 它能做什么

tech-trends 是一个跑在本地的技术情报监控系统。它从五个维度捕获技术信号，构建本地向量知识库，用 LLM 做问答和摘要，还能对技术趋势进行量化预测、回测和校准。

### 早间技术简报

```bash
tech-trends digest
```

从本地知识库提取最近 24 小时入库的内容，按主题分类，输出带摘要、分来源的结构化简报。

### 趋势预测

```bash
tech-trends forecast "rust ai infrastructure"
```

跨越 HN、arXiv、专利、书籍、GitHub 五个维度，统计关键词在 30/90/180 天窗口的变化，给出置信度分级的预测（含历史校准权重），标注技术处于 emerging / accelerating / maturing / declining 哪个阶段。

### 回测验证

```bash
tech-trends backtest "rust ai infrastructure"
```

用历史数据验证预测准确性，对比"当前窗口"与"前一个同等窗口"的变化，识别哪个数据源最先出现信号。

### 预测校准

```bash
tech-trends calibrate "rust"
```

对比历史预测记录与实际数据，计算方向准确率，输出置信度调整建议。每次 forecast 自动保存预测记录用于后续校准。

### RAG 对话

```bash
tech-trends chat
```

多轮对话模式。问题先经过向量检索找到最相关的本地文章，再送入 LLM 生成有据可查的回答。

### 话题监控

```bash
tech-trends topic create "Rust Agent" --keywords "rust,agent,autonomous"
tech-trends topic run
```

创建持续监控的话题，系统自动跑完整分析流水线（forecast + backtest），生成态势报告。

### 数据库状态

```bash
tech-trends status
```

查看各来源 story 数量、评论数、监控话题数、最近入库和最早数据时间。

## 快速开始

### 前置依赖

| 依赖 | 用途 | 安装方式 |
|------|------|---------|
| Rust 1.75+ | 编译 | [rustup.rs](https://rustup.rs) |
| Qdrant | 向量数据库 | `podman run -p 6334:6334 qdrant/qdrant` |
| Ollama | 本地 Embedding | [ollama.com](https://ollama.com) |
| DeepSeek API Key | LLM 推理 | [platform.deepseek.com](https://platform.deepseek.com) |

### 安装

```bash
git clone https://github.com/magicnight/tech-trends.git
cd tech-trends
cargo build --release
```

> **Windows 用户**：如果在 Git Bash 中编译失败（link.exe 冲突），请使用项目根目录的 `build.bat`，它会自动初始化 MSVC 环境。

### 启动依赖服务

```bash
# 启动 Qdrant
podman run -d -p 6334:6334 -p 6333:6333 qdrant/qdrant

# 拉取 Embedding 模型
ollama pull nomic-embed-text
```

### 配置

在项目目录创建 `.env` 文件（启动时自动加载）：

```bash
TECT_LLM_API_KEY=sk-your-deepseek-key
```

> 其他配置项见 [配置说明](docs/configuration.md)

### 首次数据同步

```bash
# 同步全部数据源（默认各 30 条），含向量化
tech-trends sync all

# 或单独同步某个来源
tech-trends sync hn --limit 50
tech-trends sync github --limit 20

# 仅写入 SQLite，跳过向量化
tech-trends sync all --skip-index
```

## 数据来源

每个来源代表技术生命周期的不同阶段：

| 来源 | 信号类型 | 生命周期位置 | API |
|------|---------|-------------|-----|
| Hacker News | 社区讨论 + 评论（弱信号） | 最早期 | Firebase API |
| GitHub | 开源项目热度（工程信号） | 探索期 | Search API |
| arXiv | 学术论文（研究方向） | 研究期 | Export API (Atom/XML) |
| 美国专利 | 专利申请（产业落地） | 成长期 | PatentsView API |
| 技术书籍 | 知识沉淀（体系化传播） | 成熟期 | Google Books API |

五个信号叠加，画出一条技术从 "emerging" 到 "maturing" 的完整轨迹。

### 书籍出版社适配器

书籍爬虫为不同出版社提供专用查询策略：

| 出版社 | 适配器策略 |
|--------|-----------|
| Manning | 精确匹配 + "in Action/Practice" 标题特征 |
| O'Reilly | 精确匹配 "O'Reilly Media" 出版社 |
| Packt | 精确匹配 "Packt Publishing" |
| 其他 | Google Books API 通用 fallback |

## 项目结构

```
src/
├── main.rs              # CLI 入口：10 个命令的分发与 I/O
├── lib.rs               # 公共 API（供 Tauri 桌面端复用）
├── config.rs            # 环境变量配置（.env 自动加载）
├── models/
│   ├── story.rs         # 统一 Story 模型 + Source 枚举（5 种来源）
│   └── topic.rs         # Topic + TrendStage + Confidence
├── db/
│   └── schema.rs        # SQLite 建表迁移（5 张表）
├── crawlers/
│   ├── mod.rs           # Crawler trait 定义
│   ├── hn.rs            # HN 爬虫（并发 + 增量 + BFS 评论）
│   ├── arxiv.rs         # arXiv 轻量 XML 解析
│   ├── patent.rs        # PatentsView API
│   ├── book.rs          # Google Books + 出版社适配器模式
│   └── github.rs        # GitHub Search API（热门仓库）
├── vector/
│   ├── embedding.rs     # Ollama embedding 客户端
│   └── store.rs         # Qdrant 向量存储
├── llm/
│   └── client.rs        # OpenAI 兼容 LLM 客户端
└── services/
    ├── indexer.rs       # 向量索引管道（FNV-1a ID 映射）
    ├── digest.rs        # 每日技术简报
    ├── forecast.rs      # 趋势预测（置信度 + 阶段判断）
    ├── backtest.rs      # 回测分析（窗口对比 + 领先信号源）
    ├── calibration.rs   # 预测校准（历史对比 + 置信度调整）
    ├── chat.rs          # RAG 对话引擎
    └── topic.rs         # 话题 CRUD + 分析流水线
```

## 文档

- [CLI 命令参考](docs/cli-reference.md) — 完整的 10 个命令用法
- [配置说明](docs/configuration.md) — 所有环境变量和默认值
- [架构设计](docs/architecture.md) — 系统架构与核心设计决策
- [部署指南](docs/deployment.md) — 本地开发、生产部署、Podman 方案
- [贡献指南](CONTRIBUTING.md) — 如何参与开发

## 技术选型

| 技术 | 选择理由 |
|------|---------|
| Rust | 性能好、依赖管理优秀、CLI + lib 复用天然支持 |
| SQLite | 本地优先、零运维、嵌入式。`rusqlite` 的 `bundled` feature 让部署零依赖 |
| Qdrant | 成熟向量数据库，Podman 一行启动，gRPC 客户端性能好 |
| Ollama | 本地 LLM/Embedding 推理，隐私安全，零成本 |
| clap | Rust 生态最成熟的 CLI 框架，derive 宏体验好 |
| DeepSeek | 性价比高，中文能力强，OpenAI 兼容格式通用性好 |

## Roadmap

- [x] GitHub Trending 数据源接入
- [x] 出版社专用适配器（Manning / O'Reilly / Packt）
- [x] forecast 校准机制（历史回测自动调整置信度模型）
- [x] 向量索引管道（sync 后自动 embed + upsert）
- [x] HN 评论 BFS 爬取 + 向量化
- [x] reindex 命令（从 SQLite 重建 Qdrant）
- [x] status 命令（数据库统计）
- [x] .env 文件自动加载
- [ ] ratatui TUI 仪表盘
- [ ] MCP Server（让 AI Agent 直接调用 tech-trends）
- [ ] per-source 调度间隔
- [ ] Tauri 桌面客户端

## License

MIT
