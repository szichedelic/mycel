use anyhow::Result;
use clap::{Parser, Subcommand};

mod bank;
mod cli;
mod config;
mod db;
mod session;
mod tui;
mod worktree;

#[derive(Parser)]
#[command(name = "mycel")]
#[command(about = "The network beneath your code", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Register current directory as a project
    Init,
    /// List all registered projects
    Projects,
    /// Create a new worktree with a Claude session
    Spawn {
        /// Name for the worktree/session
        name: String,
    },
    /// Attach to an existing session
    Attach {
        /// Name of the session to attach to
        name: String,
    },
    /// List worktrees/sessions in current project
    List,
    /// Kill a session and optionally remove worktree
    Kill {
        /// Name of the session to kill
        name: String,
        /// Also remove the worktree
        #[arg(short, long)]
        remove: bool,
    },
    /// Bank a completed session (bundle commits for later)
    Bank {
        /// Name of the session to bank
        name: String,
        /// Keep the session running (don't kill/remove)
        #[arg(short, long)]
        keep: bool,
    },
    /// Restore a banked session
    Unbank {
        /// Name of the banked session
        name: String,
        /// Also spawn a new session
        #[arg(short, long)]
        spawn: bool,
    },
    /// List banked sessions
    Banked,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        None => tui::run().await,
        Some(Commands::Init) => cli::init::run().await,
        Some(Commands::Projects) => cli::projects::run().await,
        Some(Commands::Spawn { name }) => cli::spawn::run(&name).await,
        Some(Commands::Attach { name }) => cli::attach::run(&name).await,
        Some(Commands::List) => cli::list::run().await,
        Some(Commands::Kill { name, remove }) => cli::kill::run(&name, remove).await,
        Some(Commands::Bank { name, keep }) => cli::bank::run(&name, keep).await,
        Some(Commands::Unbank { name, spawn }) => cli::unbank::run(&name, spawn).await,
        Some(Commands::Banked) => cli::banked::run().await,
    }
}
