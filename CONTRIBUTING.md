# 贡献指南

感谢你对 tech-trends 的兴趣！以下是参与开发的指引。

## 开发环境搭建

```bash
# 克隆仓库
git clone https://github.com/magicnight/tech-trends.git
cd tech-trends

# 编译（确保有 Rust 1.75+）
cargo build

# 运行测试
cargo test

# 代码检查
cargo clippy -- -W clippy::all
```

## 项目架构

```
src/
├── main.rs          # CLI 入口，10 个命令的分发与 I/O
├── lib.rs           # 公共模块导出
├── config.rs        # 环境变量配置（.env 自动加载）
├── models/          # 数据模型（Story, Topic, Source, Comment）
├── db/              # SQLite 数据库层（5 张表）
├── crawlers/        # 5 个数据源爬虫（实现 Crawler trait）
│   ├── hn.rs        # HN（含 BFS 评论）
│   ├── arxiv.rs     # arXiv 论文
│   ├── patent.rs    # 美国专利
│   ├── book.rs      # 技术书籍（出版社适配器模式）
│   └── github.rs    # GitHub 热门仓库
├── vector/          # 向量化（Ollama embedding + Qdrant）
├── llm/             # LLM 客户端（OpenAI 兼容）
└── services/        # 业务逻辑
    ├── indexer.rs    # 向量索引管道（FNV-1a ID 映射）
    ├── digest.rs     # 每日技术简报
    ├── forecast.rs   # 趋势预测
    ├── backtest.rs   # 回测分析
    ├── calibration.rs# 预测校准
    ├── chat.rs       # RAG 对话引擎
    └── topic.rs      # 话题管理 + 分析流水线
```

**关键原则**：`main.rs` 不包含业务逻辑，所有功能通过 `lib.rs` 导出。这使得 Tauri 桌面端和未来的 MCP Server 可以直接复用。

## 添加新数据源

1. 在 `src/crawlers/` 下创建新文件（如 `reddit.rs`）
2. 实现 `Crawler` trait：

```rust
#[async_trait]
impl Crawler for RedditCrawler {
    fn source_name(&self) -> &'static str { "reddit" }
    async fn fetch(&self, limit: usize) -> Result<Vec<Story>> { ... }
}
```

3. 在 `src/crawlers/mod.rs` 中添加 `pub mod reddit;`
4. 在 `src/models/story.rs` 的 `Source` 枚举中添加新变体（含 `as_str` 和 `from_str`）
5. 在 `main.rs` 的 `cmd_sync` 中注册新爬虫

已有的 5 个数据源（HN, arXiv, Patent, Book, GitHub）都遵循此模式。

## 添加新出版社适配器

`src/crawlers/book.rs` 使用适配器模式。添加新出版社：

1. 在 `BookCrawler` 中实现专用方法（如 `search_pragmatic`）
2. 在 `search_publisher` 的 match 中添加路由
3. Google Books API 始终作为 fallback

已有的适配器：Manning、O'Reilly、Packt 各有专用查询策略。

## 代码规范

- 使用 `anyhow::Result` 做错误传播
- 使用 `tracing` 宏做日志（`tracing::info!`, `tracing::warn!`）
- 公共 API 加 doc comment（`///`）
- 保持模块边界清晰：crawlers 不直接访问 db，通过 services 协调

## 提交规范

```
feat: 添加 Reddit 数据源
fix: 修复 arXiv XML 解析在缺少 author 时 panic
refactor: 抽取 Crawler trait 到独立模块
docs: 更新 CLI 命令参考
```

## 测试

```bash
# 运行全部测试
cargo test

# 运行特定模块的测试
cargo test --lib models

# 带日志输出
RUST_LOG=debug cargo test -- --nocapture
```

## 发布流程

1. 更新 `Cargo.toml` 中的版本号
2. 更新 CHANGELOG
3. `cargo build --release` 验证编译
4. 创建 git tag：`git tag v0.x.x`
5. 推送：`git push --tags`
