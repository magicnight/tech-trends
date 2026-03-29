use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use tracing_subscriber::EnvFilter;

use tech_trends::config::Config;
use tech_trends::crawlers::{
    arxiv::ArxivCrawler, book::BookCrawler, github::GitHubCrawler, hn::HnCrawler,
    patent::PatentCrawler, Crawler,
};
use tech_trends::db::Database;
use tech_trends::llm::LlmClient;
use tech_trends::services::{backtest, calibration, chat, digest, forecast, indexer, topic};
use tech_trends::vector::{EmbeddingClient, VectorStore};

#[derive(Parser)]
#[command(name = "tech-trends", version, about = "本地优先的 AI 驱动技术雷达")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 同步数据源（含向量化）
    Sync {
        /// 指定来源: hn, arxiv, patent, book, github, all
        #[arg(default_value = "all")]
        source: String,
        /// 每个来源抓取条数上限
        #[arg(short, long, default_value = "30")]
        limit: usize,
        /// 跳过向量化（仅写入 SQLite）
        #[arg(long, default_value = "false")]
        skip_index: bool,
    },
    /// 从 SQLite 重建 Qdrant 向量索引
    Reindex {
        /// 批次大小
        #[arg(short, long, default_value = "50")]
        batch_size: usize,
    },
    /// 生成每日技术简报
    Digest,
    /// 趋势预测
    Forecast {
        /// 要预测的关键词
        keyword: String,
    },
    /// 回测分析
    Backtest {
        /// 要回测的关键词
        keyword: String,
    },
    /// 进入多轮 RAG 对话模式
    Chat,
    /// 话题管理
    Topic {
        #[command(subcommand)]
        action: TopicAction,
    },
    /// 校准预测模型（对比历史预测与实际数据）
    Calibrate {
        /// 要校准的关键词
        keyword: String,
    },
    /// 显示数据库统计信息
    Status,
}

#[derive(Subcommand)]
enum TopicAction {
    /// 创建新话题
    Create {
        /// 话题名称
        name: String,
        /// 关键词列表（逗号分隔）
        #[arg(short, long)]
        keywords: String,
    },
    /// 列出所有话题
    List,
    /// 运行话题分析流水线
    Run {
        /// 话题名称（不指定则运行全部已启用话题）
        name: Option<String>,
    },
}

fn print_banner() {
    let banner = r#"
    ╔════════════════════════════════════════════════════════════════╗
    ║                                                                ║
    ║    ████████╗███████╗ ██████╗██╗  ██╗                           ║
    ║    ╚══██╔══╝██╔════╝██╔════╝██║  ██║                           ║
    ║       ██║   █████╗  ██║     ███████║                           ║
    ║       ██║   ██╔══╝  ██║     ██╔══██║                           ║
    ║       ██║   ███████╗╚██████╗██║  ██║                           ║
    ║       ╚═╝   ╚══════╝ ╚═════╝╚═╝  ╚═╝                           ║
    ║    ████████╗██████╗ ███████╗███╗   ██╗██████╗ ███████╗         ║
    ║    ╚══██╔══╝██╔══██╗██╔════╝████╗  ██║██╔══██╗██╔════╝         ║
    ║       ██║   ██████╔╝█████╗  ██╔██╗ ██║██║  ██║███████╗         ║
    ║       ██║   ██╔══██╗██╔══╝  ██║╚██╗██║██║  ██║╚════██║         ║
    ║       ██║   ██║  ██║███████╗██║ ╚████║██████╔╝███████║         ║
    ║       ╚═╝   ╚═╝  ╚═╝╚══════╝╚═╝  ╚═══╝╚═════╝ ╚══════╝         ║
    ║                                                                ║
    ║         ⚡ AI-Powered Tech Radar — Local First ⚡              ║
    ║    ─────────────────────────────────────────────────           ║
    ║     HN · arXiv · Patents · Books · GitHub                      ║
    ║                                                                ║
    ╚════════════════════════════════════════════════════════════════╝
"#;
    println!("{}", banner.cyan());
}

#[tokio::main]
async fn main() -> Result<()> {
    // 加载 .env 文件
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    print_banner();

    let cli = Cli::parse();
    let cfg = Config::from_env();
    let db = Database::open(&cfg.db_path)?;

    match cli.command {
        Commands::Sync {
            source,
            limit,
            skip_index,
        } => {
            cmd_sync(&cfg, &db, &source, limit, skip_index).await?;
        }
        Commands::Reindex { batch_size } => {
            cmd_reindex(&cfg, &db, batch_size).await?;
        }
        Commands::Digest => {
            let llm = make_llm(&cfg);
            let result = digest::generate_digest(&db, &llm).await?;
            println!("{result}");
        }
        Commands::Forecast { keyword } => {
            let llm = make_llm(&cfg);
            let result = forecast::forecast(&db, &llm, &keyword).await?;

            // 保存预测记录用于校准
            let _ = calibration::save_prediction(
                &db,
                &keyword,
                result.stage,
                result.confidence,
                result.windows.days_30,
                result.windows.days_90,
                result.windows.days_180,
            );

            // 显示校准权重（如有历史数据）
            let cal_weight = calibration::get_calibrated_weight(&db, &keyword);

            println!("{}", "━".repeat(50).dimmed());
            println!("{} {}", "关键词:".bold(), result.keyword);
            println!("{} {}", "阶段:".bold(), result.stage);
            println!("{} {}", "置信度:".bold(), result.confidence);
            if (cal_weight - 1.0).abs() > 0.01 {
                println!(
                    "{} {:.1}x (基于历史校准)",
                    "置信度调整:".bold(),
                    cal_weight
                );
            }
            println!(
                "{} 30d={}, 90d={}, 180d={}, total={}",
                "窗口统计:".bold(),
                result.windows.days_30,
                result.windows.days_90,
                result.windows.days_180,
                result.windows.total
            );
            println!("{}", "来源分布:".bold());
            for (src, count) in &result.source_breakdown {
                println!("  {src}: {count}");
            }
            if let Some(narrative) = &result.narrative {
                println!("{}", "━".repeat(50).dimmed());
                println!("{narrative}");
            }
        }
        Commands::Backtest { keyword } => {
            let result = backtest::backtest(&db, &keyword)?;
            println!("{}", "━".repeat(50).dimmed());
            println!("{} {}", "关键词:".bold(), result.keyword);
            for w in &result.windows {
                println!(
                    "  {}天窗口: 当前={}, 前期={}, delta={} {}",
                    w.window_days, w.current_count, w.previous_count, w.delta, w.direction
                );
            }
            if let Some(leading) = &result.leading_signal {
                println!("{} {leading}", "领先信号源:".bold());
            }
        }
        Commands::Chat => {
            cmd_chat(&cfg).await?;
        }
        Commands::Topic { action } => match action {
            TopicAction::Create { name, keywords } => {
                let kws: Vec<String> = keywords.split(',').map(|s| s.trim().to_string()).collect();
                let t = topic::create_topic(&db, &name, kws)?;
                println!("已创建话题: {} (id={})", t.name, t.id);
            }
            TopicAction::List => {
                let topics = topic::list_topics(&db)?;
                if topics.is_empty() {
                    println!("暂无话题。使用 `tech-trends topic create` 创建。");
                } else {
                    for t in &topics {
                        let status = if t.enabled { "✓" } else { "✗" };
                        println!(
                            "  [{status}] {} — 关键词: {}",
                            t.name.bold(),
                            t.keywords.join(", ")
                        );
                    }
                }
            }
            TopicAction::Run { name } => {
                let llm = make_llm(&cfg);
                let topics = topic::list_topics(&db)?;
                let targets: Vec<_> = if let Some(ref n) = name {
                    topics.into_iter().filter(|t| t.name == *n).collect()
                } else {
                    topics.into_iter().filter(|t| t.enabled).collect()
                };

                if targets.is_empty() {
                    println!("没有匹配的话题。");
                    return Ok(());
                }

                for t in &targets {
                    println!("{}", format!("▶ 分析话题: {}", t.name).cyan());
                    let report = topic::run_topic_pipeline(&db, &llm, t).await?;
                    println!("{report}");
                    println!();
                }
            }
        },
        Commands::Calibrate { keyword } => {
            let report = calibration::calibrate(&db, &keyword)?;
            println!("{}", "━".repeat(50).dimmed());
            println!("{} {}", "关键词:".bold(), report.keyword);
            println!("{} {}", "历史预测数:".bold(), report.total_predictions);
            if let Some(acc) = report.direction_accuracy {
                println!("{} {:.0}%", "方向准确率:".bold(), acc * 100.0);
            } else {
                println!("{} 数据不足", "方向准确率:".bold());
            }
            println!("{} {}", "置信度调整建议:".bold(), report.confidence_adjustment);
            if !report.details.is_empty() {
                println!("\n{}", "历史对比:".bold());
                for d in &report.details {
                    let mark = if d.direction_correct { "✓".green() } else { "✗".red() };
                    println!(
                        "  {mark} {} — 预测30d={}, 实际30d={}, 阶段={}",
                        d.date, d.predicted_30d, d.actual_30d, d.predicted_stage
                    );
                }
            }
            println!("{}", "━".repeat(50).dimmed());
        }
        Commands::Status => {
            cmd_status(&db)?;
        }
    }

    Ok(())
}

/// 数据同步命令（含可选向量化）
async fn cmd_sync(
    cfg: &Config,
    db: &Database,
    source: &str,
    limit: usize,
    skip_index: bool,
) -> Result<()> {
    // HN 需要特殊处理（评论）
    let is_hn = source == "hn" || source == "all";

    // 非 HN 爬虫
    let other_crawlers: Vec<Box<dyn Crawler>> = match source {
        "hn" => vec![],
        "arxiv" => vec![Box::new(ArxivCrawler::new(vec![
            "cs.AI".into(),
            "cs.LG".into(),
            "cs.SE".into(),
        ]))],
        "patent" => vec![Box::new(PatentCrawler::new(vec![
            "artificial intelligence".into(),
            "machine learning".into(),
        ]))],
        "book" => vec![Box::new(BookCrawler::new(vec![
            "Manning".into(),
            "O'Reilly".into(),
            "Packt".into(),
        ]))],
        "github" => vec![Box::new(GitHubCrawler::new(
            vec!["rust".into(), "python".into(), "typescript".into()],
            vec!["machine-learning".into(), "ai".into(), "llm".into()],
        ))],
        "all" => vec![
            Box::new(ArxivCrawler::new(vec![
                "cs.AI".into(),
                "cs.LG".into(),
                "cs.SE".into(),
            ])),
            Box::new(PatentCrawler::new(vec![
                "artificial intelligence".into(),
                "machine learning".into(),
            ])),
            Box::new(BookCrawler::new(vec![
                "Manning".into(),
                "O'Reilly".into(),
                "Packt".into(),
            ])),
            Box::new(GitHubCrawler::new(
                vec!["rust".into(), "python".into(), "typescript".into()],
                vec!["machine-learning".into(), "ai".into(), "llm".into()],
            )),
        ],
        _ => {
            anyhow::bail!("未知来源: {source}。可选: hn, arxiv, patent, book, github, all");
        }
    };

    // 初始化向量化组件（如果需要）
    let indexing = if !skip_index {
        match (
            EmbeddingClient::new(&cfg.ollama_url, &cfg.embedding_model),
            VectorStore::new(&cfg.qdrant_url, &cfg.qdrant_collection, cfg.embedding_dim).await,
        ) {
            (emb, Ok(vs)) => Some((emb, vs)),
            (_, Err(e)) => {
                println!(
                    "  {} 向量化不可用（Qdrant: {e}），仅写入 SQLite",
                    "⚠".yellow()
                );
                None
            }
        }
    } else {
        None
    };

    // 处理 HN（含评论）
    if is_hn {
        println!("{}", "▶ 同步 hackernews...".cyan());
        let hn = HnCrawler::new();
        match hn.fetch_with_comments(limit).await {
            Ok(result) => {
                let inserted = insert_stories(db, &result.stories)?;
                let comments_inserted = insert_comments(db, &result.comments)?;
                println!(
                    "  {} {} 条 story 中新增 {} 条, {} 条评论中新增 {} 条",
                    "✓".green(),
                    result.stories.len(),
                    inserted,
                    result.comments.len(),
                    comments_inserted
                );

                // 向量化新增内容
                if let Some((ref emb, ref vs)) = indexing {
                    let story_indexed = indexer::index_stories(&result.stories, emb, vs).await?;
                    let comment_indexed =
                        indexer::index_comments(&result.comments, emb, vs).await?;
                    println!(
                        "  {} 向量化: {} stories + {} comments",
                        "✓".green(),
                        story_indexed,
                        comment_indexed
                    );
                }
            }
            Err(e) => {
                println!("  {} hackernews 同步失败: {e}", "✗".red());
            }
        }
    }

    // 处理其他爬虫
    for crawler in &other_crawlers {
        let name = crawler.source_name();
        println!("{}", format!("▶ 同步 {name}...").cyan());

        match crawler.fetch(limit).await {
            Ok(stories) => {
                let inserted = insert_stories(db, &stories)?;
                println!(
                    "  {} {} 条中新增 {} 条",
                    "✓".green(),
                    stories.len(),
                    inserted
                );

                if let Some((ref emb, ref vs)) = indexing {
                    let indexed = indexer::index_stories(&stories, emb, vs).await?;
                    println!("  {} 向量化: {} 条", "✓".green(), indexed);
                }
            }
            Err(e) => {
                println!("  {} {name} 同步失败: {e}", "✗".red());
            }
        }
    }

    Ok(())
}

/// 从 SQLite 重建 Qdrant 向量索引
async fn cmd_reindex(cfg: &Config, db: &Database, batch_size: usize) -> Result<()> {
    let embedding = EmbeddingClient::new(&cfg.ollama_url, &cfg.embedding_model);
    let vector_store =
        VectorStore::new(&cfg.qdrant_url, &cfg.qdrant_collection, cfg.embedding_dim).await?;

    let conn = db.conn();

    // 统计总数
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM stories", [], |r| r.get(0))?;
    println!("共 {total} 条 story 待索引 (batch_size={batch_size})");

    let mut offset = 0i64;
    let mut total_indexed = 0usize;

    loop {
        let mut stmt = conn.prepare(
            "SELECT external_id, source, title, url, body, author, published_at, score, metadata
             FROM stories ORDER BY id LIMIT ?1 OFFSET ?2",
        )?;

        let stories: Vec<tech_trends::models::Story> = stmt
            .query_map(rusqlite::params![batch_size as i64, offset], |row| {
                let source_str: String = row.get(1)?;
                let published_str: String = row.get(6)?;
                Ok(tech_trends::models::Story {
                    external_id: row.get(0)?,
                    source: tech_trends::models::Source::from_str(&source_str)
                        .unwrap_or(tech_trends::models::Source::HackerNews),
                    title: row.get(2)?,
                    url: row.get(3)?,
                    body: row.get(4)?,
                    author: row.get(5)?,
                    published_at: published_str
                        .parse()
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    score: row.get(7)?,
                    metadata: row
                        .get::<_, Option<String>>(8)?
                        .and_then(|s| serde_json::from_str(&s).ok()),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        if stories.is_empty() {
            break;
        }

        let batch_len = stories.len();
        let indexed = indexer::index_stories(&stories, &embedding, &vector_store).await?;
        total_indexed += indexed;
        offset += batch_len as i64;

        println!(
            "  进度: {offset}/{total} — 本批索引 {indexed}/{batch_len}",
        );
    }

    // 评论
    let comment_total: i64 = conn.query_row("SELECT COUNT(*) FROM comments", [], |r| r.get(0))?;
    if comment_total > 0 {
        println!("共 {comment_total} 条评论待索引");
        let mut stmt = conn.prepare(
            "SELECT external_id, story_external_id, text, author, published_at FROM comments",
        )?;
        let comments: Vec<tech_trends::models::Comment> = stmt
            .query_map([], |row| {
                let published_str: String = row.get(4)?;
                Ok(tech_trends::models::Comment {
                    external_id: row.get::<_, i64>(0)? as u64,
                    story_external_id: row.get(1)?,
                    text: row.get(2)?,
                    author: row.get(3)?,
                    published_at: published_str
                        .parse()
                        .unwrap_or_else(|_| chrono::Utc::now()),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        let comment_indexed =
            indexer::index_comments(&comments, &embedding, &vector_store).await?;
        println!("  评论索引完成: {comment_indexed}/{comment_total}");
    }

    println!(
        "{}",
        format!("✓ 重建完成: {total_indexed} stories 已索引").green()
    );

    Ok(())
}

/// 显示数据库统计信息
fn cmd_status(db: &Database) -> Result<()> {
    let conn = db.conn();

    println!("{}", "tech-trends 数据库状态".bold());
    println!("{}", "━".repeat(40).dimmed());

    // 各来源 story 数量
    let mut stmt = conn.prepare(
        "SELECT source, COUNT(*) FROM stories GROUP BY source ORDER BY COUNT(*) DESC",
    )?;
    let rows: Vec<(String, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    let total_stories: i64 = rows.iter().map(|(_, c)| c).sum();
    println!("{} {}", "Stories 总计:".bold(), total_stories);
    for (source, count) in &rows {
        println!("  {source}: {count}");
    }

    // 评论数
    let comment_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM comments", [], |r| r.get(0))?;
    println!("{} {}", "HN 评论:".bold(), comment_count);

    // 话题数
    let topic_count: i64 = conn.query_row("SELECT COUNT(*) FROM topics", [], |r| r.get(0))?;
    println!("{} {}", "监控话题:".bold(), topic_count);

    // 最近同步时间
    let latest: Option<String> = conn
        .query_row(
            "SELECT MAX(created_at) FROM stories",
            [],
            |r| r.get(0),
        )
        .ok();
    if let Some(ts) = latest {
        println!("{} {}", "最近入库:".bold(), ts);
    }

    // 时间跨度
    let earliest: Option<String> = conn
        .query_row(
            "SELECT MIN(published_at) FROM stories",
            [],
            |r| r.get(0),
        )
        .ok();
    if let Some(ts) = earliest {
        println!("{} {}", "最早数据:".bold(), ts);
    }

    println!("{}", "━".repeat(40).dimmed());
    Ok(())
}

/// RAG 对话模式
async fn cmd_chat(cfg: &Config) -> Result<()> {
    let embedding = EmbeddingClient::new(&cfg.ollama_url, &cfg.embedding_model);
    let vector_store =
        VectorStore::new(&cfg.qdrant_url, &cfg.qdrant_collection, cfg.embedding_dim).await?;
    let llm = make_llm(cfg);

    let mut engine = chat::ChatEngine::new(&embedding, &vector_store, &llm);

    println!("{}", "tech-trends 对话模式 (输入 exit 退出)".bold());
    println!("{}", "━".repeat(50).dimmed());

    let mut rl = rustyline::DefaultEditor::new()?;
    loop {
        let readline = rl.readline("你> ");
        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if line == "exit" || line == "quit" {
                    break;
                }
                let _ = rl.add_history_entry(line);

                match engine.ask(line).await {
                    Ok(reply) => {
                        println!("\n{} {reply}\n", "AI>".green().bold());
                    }
                    Err(e) => {
                        println!("{} {e}", "错误:".red());
                    }
                }
            }
            Err(_) => break,
        }
    }

    Ok(())
}

/// 插入 stories 到 SQLite（增量）
fn insert_stories(db: &Database, stories: &[tech_trends::models::Story]) -> Result<usize> {
    let conn = db.conn();
    let mut inserted = 0;
    for story in stories {
        let result = conn.execute(
            "INSERT OR IGNORE INTO stories
             (external_id, source, title, url, body, author, published_at, score, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                story.external_id,
                story.source.as_str(),
                story.title,
                story.url,
                story.body,
                story.author,
                story.published_at.to_rfc3339(),
                story.score,
                story.metadata.as_ref().map(|m| m.to_string()),
            ],
        );
        if let Ok(1) = result {
            inserted += 1;
        }
    }
    Ok(inserted)
}

/// 插入 HN 评论到 SQLite（增量）
fn insert_comments(db: &Database, comments: &[tech_trends::models::Comment]) -> Result<usize> {
    let conn = db.conn();
    let mut inserted = 0;
    for comment in comments {
        let result = conn.execute(
            "INSERT OR IGNORE INTO comments
             (external_id, story_external_id, text, author, published_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                comment.external_id as i64,
                comment.story_external_id,
                comment.text,
                comment.author,
                comment.published_at.to_rfc3339(),
            ],
        );
        if let Ok(1) = result {
            inserted += 1;
        }
    }
    Ok(inserted)
}

fn make_llm(cfg: &Config) -> LlmClient {
    LlmClient::new(&cfg.llm_api_url, &cfg.llm_api_key, &cfg.llm_model)
}
