use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod bank;
mod cli;
mod config;
mod confirm;
mod db;
mod disk;
mod notify;
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
    /// Create a new worktree with an AI session
    Spawn {
        /// Name for the worktree/session
        name: String,
        /// Optional note for the session
        #[arg(short, long)]
        note: Option<String>,
        /// Optional session template name
        #[arg(short, long)]
        template: Option<String>,
        /// Optional backend override (e.g. claude, codex)
        #[arg(short, long)]
        backend: Option<String>,
    },
    /// Attach to an existing session
    Attach {
        /// Name of the session to attach to
        name: String,
    },
    /// List worktrees/sessions in current project
    List,
    /// List session history in current project
    History,
    /// Kill a session and optionally remove worktree
    Kill {
        /// Name of the session to kill
        name: String,
        /// Also remove the worktree
        #[arg(short, long)]
        remove: bool,
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
    /// Stop a session without removing its worktree
    Stop {
        /// Name of the session to stop
        name: String,
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
    /// Bank a completed session (bundle commits for later)
    Bank {
        /// Name of the session to bank
        name: String,
        /// Keep the session running (don't kill/remove)
        #[arg(short, long)]
        keep: bool,
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
    /// Restore a banked session
    Unbank {
        /// Name of the banked session
        name: String,
        /// Also spawn a new session
        #[arg(short, long)]
        spawn: bool,
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
    /// List banked sessions
    Banked,
    /// Export a banked session to a portable archive
    BankExport {
        /// Name of the banked session
        name: String,
        /// Output path for the archive
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Import a banked session from a portable archive
    BankImport {
        /// Path to the archive file
        path: PathBuf,
        /// Optional session name override
        #[arg(short, long)]
        name: Option<String>,
        /// Overwrite existing bundle
        #[arg(long)]
        force: bool,
    },
    /// Send desktop notifications for session events
    Notify {
        /// Poll interval in seconds
        #[arg(short, long, default_value_t = 5)]
        interval: u64,
    },
    /// Serve the TUI over a local web server
    Web {
        /// Host to bind (use 0.0.0.0 to reach from your phone)
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Port to bind
        #[arg(short, long, default_value_t = 3799)]
        port: u16,
        /// Optional access token to protect the web UI
        #[arg(long)]
        token: Option<String>,
        /// TLS certificate (PEM) for HTTPS
        #[arg(long)]
        tls_cert: Option<PathBuf>,
        /// TLS private key (PEM) for HTTPS
        #[arg(long)]
        tls_key: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        None => tui::run().await,
        Some(Commands::Init) => cli::init::run().await,
        Some(Commands::Projects) => cli::projects::run().await,
        Some(Commands::Spawn {
            name,
            note,
            template,
            backend,
        }) => {
            cli::spawn::run(
                &name,
                note.as_deref(),
                template.as_deref(),
                backend.as_deref(),
            )
            .await
        }
        Some(Commands::Attach { name }) => cli::attach::run(&name).await,
        Some(Commands::List) => cli::list::run().await,
        Some(Commands::History) => cli::history::run().await,
        Some(Commands::Kill {
            name,
            remove,
            force,
        }) => cli::kill::run(&name, remove, force).await,
        Some(Commands::Stop { name, force }) => cli::stop::run(&name, force).await,
        Some(Commands::Bank { name, keep, force }) => cli::bank::run(&name, keep, force).await,
        Some(Commands::Unbank { name, spawn, force }) => {
            cli::unbank::run(&name, spawn, force).await
        }
        Some(Commands::Banked) => cli::banked::run().await,
        Some(Commands::BankExport { name, output }) => cli::bank_export::run(&name, output).await,
        Some(Commands::BankImport { path, name, force }) => {
            cli::bank_import::run(&path, name.as_deref(), force).await
        }
        Some(Commands::Notify { interval }) => cli::notify::run(interval).await,
        Some(Commands::Web {
            host,
            port,
            token,
            tls_cert,
            tls_key,
        }) => {
            cli::web::run(
                &host,
                port,
                token.as_deref(),
                tls_cert.as_ref(),
                tls_key.as_ref(),
            )
            .await
        }
    }
}
