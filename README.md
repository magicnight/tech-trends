# tect-brain

本地优先的 AI 驱动技术雷达 — 从 Hacker News 到 arXiv 论文，从专利数据到技术书籍。

## 它能做什么

tect-brain 是一个跑在本地的技术情报监控系统。它从四个维度捕获技术信号，构建本地向量知识库，用 LLM 做问答和摘要，还能对技术趋势进行量化预测和回测。

### 早间技术简报

```bash
tect-brain digest
```

从本地知识库提取最近 24 小时入库的内容，按主题分类，输出带摘要、分来源的结构化简报。

### 趋势预测

```bash
tect-brain forecast "rust ai infrastructure"
```

跨越 HN、arXiv、专利、书籍四个维度，统计关键词在 30/90/180 天窗口的变化，给出置信度分级的预测，标注技术处于 emerging / accelerating / maturing / declining 哪个阶段。

### 回测验证

```bash
tect-brain backtest "rust ai infrastructure"
```

用历史数据验证预测准确性，对比"当前窗口"与"前一个同等窗口"的变化，识别哪个数据源最先出现信号。

### RAG 对话

```bash
tect-brain chat
```

多轮对话模式。问题先经过向量检索找到最相关的本地文章，再送入 LLM 生成有据可查的回答。

### 话题监控

```bash
tect-brain topic create "Rust Agent" --keywords "rust,agent,autonomous"
tect-brain topic run
```

创建持续监控的话题，系统自动跑完整分析流水线（forecast + backtest），生成态势报告。

## 快速开始

### 前置依赖

| 依赖 | 用途 | 安装方式 |
|------|------|---------|
| Rust 1.75+ | 编译 | [rustup.rs](https://rustup.rs) |
| Qdrant | 向量数据库 | `docker run -p 6334:6334 qdrant/qdrant` |
| Ollama | 本地 Embedding | [ollama.com](https://ollama.com) |
| DeepSeek API Key | LLM 推理 | [platform.deepseek.com](https://platform.deepseek.com) |

### 安装

```bash
git clone https://github.com/yourname/tect-brain.git
cd tect-brain
cargo build --release
```

> **Windows 用户**：如果在 Git Bash 中编译失败（link.exe 冲突），请使用项目根目录的 `build.bat`，它会自动初始化 MSVC 环境。

### 启动依赖服务

```bash
# 启动 Qdrant
docker run -d -p 6334:6334 -p 6333:6333 qdrant/qdrant

# 拉取 Embedding 模型
ollama pull nomic-embed-text
```

### 配置

通过环境变量配置（或写入 `.env` 文件）：

```bash
export TECT_LLM_API_KEY="sk-your-deepseek-key"
# 其他配置项见 docs/configuration.md
```

### 首次数据同步

```bash
# 同步全部数据源（默认各 30 条）
tect-brain sync all

# 或单独同步某个来源
tect-brain sync hn --limit 50
```

## 数据来源

每个来源代表技术生命周期的不同阶段：

| 来源 | 信号类型 | 生命周期位置 | API |
|------|---------|-------------|-----|
| Hacker News | 社区讨论（弱信号） | 最早期 | Firebase API |
| arXiv | 学术论文（研究方向） | 探索期 | Export API (Atom/XML) |
| 美国专利 | 专利申请（产业落地） | 成长期 | PatentsView API |
| 技术书籍 | 知识沉淀（体系化传播） | 成熟期 | Google Books API |

四个信号叠加，画出一条技术从 "emerging" 到 "maturing" 的完整轨迹。

## 项目结构

```
src/
├── main.rs              # CLI 入口：命令分发、REPL
├── lib.rs               # 公共 API（供 Tauri 桌面端复用）
├── config.rs            # 环境变量配置
├── models/
│   ├── story.rs         # 统一 Story 模型 + Source 枚举
│   └── topic.rs         # Topic + TrendStage + Confidence
├── db/
│   └── schema.rs        # SQLite 建表迁移（4 张表）
├── crawlers/
│   ├── mod.rs           # Crawler trait 定义
│   ├── hn.rs            # HN 爬虫（并发 + 增量）
│   ├── arxiv.rs         # arXiv 轻量 XML 解析
│   ├── patent.rs        # PatentsView API
│   └── book.rs          # Google Books + 适配器模式
├── vector/
│   ├── embedding.rs     # Ollama embedding 客户端
│   └── store.rs         # Qdrant 向量存储
├── llm/
│   └── client.rs        # OpenAI 兼容 LLM 客户端
└── services/
    ├── digest.rs        # 每日技术简报
    ├── forecast.rs      # 趋势预测（置信度 + 阶段判断）
    ├── backtest.rs      # 回测分析（窗口对比 + 领先信号源）
    ├── chat.rs          # RAG 对话引擎
    └── topic.rs         # 话题 CRUD + 分析流水线
```

## 文档

- [CLI 命令参考](docs/cli-reference.md) — 完整的命令行用法
- [配置说明](docs/configuration.md) — 所有环境变量和默认值
- [架构设计](docs/architecture.md) — 系统架构与核心设计决策
- [部署指南](docs/deployment.md) — 本地开发、生产部署、Docker 方案
- [贡献指南](CONTRIBUTING.md) — 如何参与开发

## 技术选型

| 技术 | 选择理由 |
|------|---------|
| Rust | 性能好、依赖管理优秀、CLI + lib 复用天然支持 |
| SQLite | 本地优先、零运维、嵌入式。`rusqlite` 的 `bundled` feature 让部署零依赖 |
| Qdrant | 成熟向量数据库，Docker 一行启动，gRPC 客户端性能好 |
| Ollama | 本地 LLM/Embedding 推理，隐私安全，零成本 |
| clap | Rust 生态最成熟的 CLI 框架，derive 宏体验好 |
| DeepSeek | 性价比高，中文能力强，OpenAI 兼容格式通用性好 |

## Roadmap

- [ ] GitHub Trending 数据源接入
- [ ] 更多出版社专用适配器（O'Reilly、Packt）
- [ ] forecast 校准机制（历史回测自动调整置信度模型）
- [ ] ratatui TUI 仪表盘
- [ ] MCP Server（让 AI Agent 直接调用 tect-brain）
- [ ] per-source 调度间隔
- [ ] Tauri 桌面客户端

## License

MIT
