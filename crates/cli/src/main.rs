//! `learning-tool`: the MCP server Claude Code uses for study sessions, plus admin commands.

mod mcp;

use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::Context;
use chrono::Utc;
use clap::{Parser, Subcommand};
use learning_core::db::{self, DataDir};
use learning_core::settings::{self, Settings};
use learning_core::{backup, scheduler, stats};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser)]
#[command(version, about = "Spaced-repetition learning tool (data in $LEARNING_TOOL_HOME or ~/.learning-tool)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Serve the MCP tools over stdio (Claude Code starts this).
    Mcp,
    /// Create or migrate the database.
    Migrate,
    /// Print today's study numbers.
    Stats,
    /// Take a backup now.
    Backup,
    /// Replace the database with a backup. Quit Claude Code first.
    Restore {
        file: PathBuf,
        /// Confirm replacing the current database.
        #[arg(long)]
        yes: bool,
    },
    /// Recompute scheduling state from the review log.
    RebuildState,
    /// Show or change settings.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show one setting, or all of them.
    Get { key: Option<String> },
    /// Change a setting.
    Set { key: String, value: String },
}

fn init_logging(dirs: &DataDir, to_file: bool) {
    let filter = EnvFilter::try_from_env("LEARNING_TOOL_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let stderr = fmt::layer().with_writer(std::io::stderr);
    let file = to_file
        .then(|| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dirs.logs_dir().join("learning-tool.log"))
                .ok()
        })
        .flatten()
        .map(|f| fmt::layer().with_ansi(false).with_writer(Mutex::new(f)));
    tracing_subscriber::registry().with(filter).with(stderr).with(file).init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let dirs = DataDir::from_env();
    dirs.ensure().with_context(|| format!("creating {}", dirs.root.display()))?;
    init_logging(&dirs, matches!(cli.command, Command::Mcp));

    if let Command::Restore { file, yes } = &cli.command {
        if !yes {
            anyhow::bail!("this replaces {}; quit Claude Code, then re-run with --yes", dirs.db_path().display());
        }
        backup::restore(&dirs, file).await?;
        eprintln!("Restored {} (previous database kept beside it).", file.display());
        return Ok(());
    }

    let pool = db::open(&dirs.db_path()).await?;
    match cli.command {
        Command::Mcp => {
            let s = Settings::load(&pool).await?;
            if let Some(path) = backup::daily_if_needed(&pool, &dirs, &s, Utc::now()).await? {
                tracing::info!("daily backup written to {}", path.display());
            }
            mcp::serve(pool, dirs).await?;
        }
        Command::Migrate => println!("Database ready at {}", dirs.db_path().display()),
        Command::Stats => {
            let s = Settings::load(&pool).await?;
            let t = stats::today(&pool, &s, Utc::now()).await?;
            println!("Study day       {}", t.study_day);
            println!("Answered        {}", t.answered_today);
            println!("Remaining       {}", t.remaining_today);
            println!("New today       {}", t.new_introduced_today);
            println!("Due tomorrow    {}", t.due_tomorrow);
            println!("Streak          {}", t.streak);
        }
        Command::Backup => {
            let s = Settings::load(&pool).await?;
            let path = backup::backup(&pool, &dirs, &s, "manual", Utc::now()).await?;
            println!("Backup written to {}", path.display());
        }
        Command::RebuildState => {
            let s = Settings::load(&pool).await?;
            let n = scheduler::rebuild_all(&pool, &s).await?;
            println!("Rebuilt scheduling state for {n} questions.");
        }
        Command::Config { action: ConfigAction::Get { key } } => {
            for (k, v) in settings::all(&pool).await? {
                if key.as_deref().is_none_or(|want| want == k) {
                    println!("{k:<28} {v}");
                }
            }
        }
        Command::Config { action: ConfigAction::Set { key, value } } => {
            settings::set(&pool, &key, &value).await?;
            println!("{key} = {value}");
        }
        Command::Restore { .. } => unreachable!("handled above"),
    }
    Ok(())
}
