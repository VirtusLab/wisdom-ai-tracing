use clap::Parser;
use std::env;

mod api_client;
mod commands;
mod config;
mod credentials;
mod hooks;

#[derive(Parser)]
#[command(name = "tracevault", version, about = "AI code governance platform")]
enum Cli {
    /// Initialize TraceVault in current repository
    Init {
        /// TraceVault server URL for repo registration
        #[arg(long)]
        server_url: Option<String>,
    },
    /// Show current session status
    Status,
    /// Handle Claude Code hook event (reads JSON from stdin)
    Hook {
        #[arg(long)]
        event: String,
    },
    /// Stream hook events to server in real-time
    Stream {
        #[arg(long)]
        event: String,
    },
    /// Check session policies before pushing
    Check,
    /// Sync repo remote URL with the TraceVault server
    Sync,
    /// Show local session statistics
    Stats,
    /// Log in to a TraceVault server
    Login {
        /// TraceVault server URL
        #[arg(long)]
        server_url: String,
    },
    /// Log out from the TraceVault server
    Logout,
    /// Push commit metadata to server (called from post-commit hook)
    CommitPush,
    /// Force-sync all pending events to server
    Flush,
    /// Verify commits are registered and sealed on the TraceVault server
    Verify {
        /// Comma-separated list of commit SHAs
        #[arg(long)]
        commits: Option<String>,
        /// Git commit range (e.g. abc1234..def5678)
        #[arg(long)]
        range: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli {
        Cli::Init { server_url } => {
            let cwd = env::current_dir().expect("Cannot determine current directory");
            match commands::init::init_in_directory(&cwd, server_url.as_deref()).await {
                Ok(()) => {
                    println!("TraceVault initialized in {}", cwd.display());
                    println!("Claude Code hooks installed in .claude/settings.json");
                    println!("Git pre-push hook installed");
                }
                Err(e) => eprintln!("Error: {e}"),
            }
        }
        Cli::Status => println!("tracevault status - not yet implemented"),
        Cli::Hook { event: _ } => {
            let cwd = env::current_dir().expect("Cannot determine current directory");
            if let Err(e) = commands::hook::handle_hook_from_stdin(&cwd) {
                eprintln!("Hook error: {e}");
            }
        }
        Cli::Stream { event } => {
            let cwd = env::current_dir().expect("Cannot determine current directory");
            if let Err(e) = commands::stream::run_stream(&cwd, &event).await {
                eprintln!("Stream error: {e}");
            }
        }
        Cli::Check => {
            let cwd = env::current_dir().expect("Cannot determine current directory");
            if let Err(e) = commands::check::check_policies(&cwd).await {
                eprintln!("Check error: {e}");
                std::process::exit(1);
            }
        }
        Cli::Sync => {
            let cwd = env::current_dir().expect("Cannot determine current directory");
            if let Err(e) = commands::sync::sync_repo(&cwd).await {
                eprintln!("Sync error: {e}");
            }
        }
        Cli::Stats => {
            let cwd = env::current_dir().expect("Cannot determine current directory");
            if let Err(e) = commands::stats::show_stats(&cwd) {
                eprintln!("Stats error: {e}");
            }
        }
        Cli::Login { server_url } => {
            if let Err(e) = commands::login::login(&server_url).await {
                eprintln!("Login error: {e}");
            }
        }
        Cli::Logout => {
            if let Err(e) = commands::logout::logout().await {
                eprintln!("Logout error: {e}");
            }
        }
        Cli::CommitPush => {
            let cwd = env::current_dir().expect("Cannot determine current directory");
            if let Err(e) = commands::commit_push::run_commit_push(&cwd).await {
                eprintln!("Commit push error: {e}");
            }
        }
        Cli::Flush => {
            let cwd = env::current_dir().expect("Cannot determine current directory");
            if let Err(e) = commands::flush::run_flush(&cwd).await {
                eprintln!("Flush error: {e}");
            }
        }
        Cli::Verify { commits, range } => {
            let cwd = env::current_dir().expect("Cannot determine current directory");
            if let Err(e) =
                commands::verify::verify(&cwd, commits.as_deref(), range.as_deref()).await
            {
                eprintln!("Verify error: {e}");
                std::process::exit(1);
            }
        }
    }
}
