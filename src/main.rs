use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use tracing_subscriber::EnvFilter;

use tect_brain::config::Config;
use tect_brain::crawlers::{
    arxiv::ArxivCrawler, book::BookCrawler, hn::HnCrawler, patent::PatentCrawler, Crawler,
};
use tect_brain::db::Database;
use tect_brain::llm::LlmClient;
use tect_brain::services::{backtest, chat, digest, forecast, topic};
use tect_brain::vector::{EmbeddingClient, VectorStore};

#[derive(Parser)]
#[command(name = "tect-brain", version, about = "本地优先的 AI 驱动技术雷达")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 同步数据源
    Sync {
        /// 指定来源: hn, arxiv, patent, book, all
        #[arg(default_value = "all")]
        source: String,
        /// 每个来源抓取条数上限
        #[arg(short, long, default_value = "30")]
        limit: usize,
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

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let cfg = Config::from_env();
    let db = Database::open(&cfg.db_path)?;

    match cli.command {
        Commands::Sync { source, limit } => {
            cmd_sync(&db, &source, limit).await?;
        }
        Commands::Digest => {
            let llm = make_llm(&cfg);
            let result = digest::generate_digest(&db, &llm).await?;
            println!("{result}");
        }
        Commands::Forecast { keyword } => {
            let llm = make_llm(&cfg);
            let result = forecast::forecast(&db, &llm, &keyword).await?;
            println!("{}", "━".repeat(50).dimmed());
            println!("{} {}", "关键词:".bold(), result.keyword);
            println!("{} {}", "阶段:".bold(), result.stage);
            println!("{} {}", "置信度:".bold(), result.confidence);
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
            cmd_chat(&cfg, &db).await?;
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
                    println!("暂无话题。使用 `tect-brain topic create` 创建。");
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
    }

    Ok(())
}

/// 数据同步命令
async fn cmd_sync(db: &Database, source: &str, limit: usize) -> Result<()> {
    let crawlers: Vec<Box<dyn Crawler>> = match source {
        "hn" => vec![Box::new(HnCrawler::new())],
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
        "all" => vec![
            Box::new(HnCrawler::new()),
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
        ],
        _ => {
            anyhow::bail!("未知来源: {source}。可选: hn, arxiv, patent, book, all");
        }
    };

    for crawler in &crawlers {
        let name = crawler.source_name();
        println!("{}", format!("▶ 同步 {name}...").cyan());

        match crawler.fetch(limit).await {
            Ok(stories) => {
                let mut inserted = 0;
                let conn = db.conn();
                for story in &stories {
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
                println!(
                    "  {} {} 条中新增 {} 条",
                    "✓".green(),
                    stories.len(),
                    inserted
                );
            }
            Err(e) => {
                println!("  {} {name} 同步失败: {e}", "✗".red());
            }
        }
    }

    Ok(())
}

/// RAG 对话模式
async fn cmd_chat(cfg: &Config, _db: &Database) -> Result<()> {
    let embedding = EmbeddingClient::new(&cfg.ollama_url, &cfg.embedding_model);
    let vector_store =
        VectorStore::new(&cfg.qdrant_url, &cfg.qdrant_collection, cfg.embedding_dim).await?;
    let llm = make_llm(cfg);

    let mut engine = chat::ChatEngine::new(&embedding, &vector_store, &llm);

    println!("{}", "tect-brain 对话模式 (输入 exit 退出)".bold());
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

fn make_llm(cfg: &Config) -> LlmClient {
    LlmClient::new(&cfg.llm_api_url, &cfg.llm_api_key, &cfg.llm_model)
}
