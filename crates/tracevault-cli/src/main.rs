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
        /// Where to install Claude Code hooks: `shared` (.claude/settings.json,
        /// typically committed) or `local` (.claude/settings.local.json,
        /// personal/git-ignored). When omitted, prompts interactively if stdin
        /// is a TTY, otherwise defaults to `shared`.
        #[arg(long, value_enum)]
        claude_settings: Option<commands::init::ClaudeSettingsTarget>,
        /// Skip updating .gitignore. Use this when your project manages
        /// .gitignore separately or you want to commit the Claude settings files.
        #[arg(long)]
        no_gitignore: bool,
    },
    /// Show current session status
    Status,
    /// Stream hook events to server in real-time.
    /// Installed into .claude/settings.json by `tracevault init` and invoked
    /// by Claude Code on every tool event — not intended to be run manually.
    #[command(hide = true)]
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
        /// Do not try to open a browser; just print the URL.
        /// Useful inside Docker / CI / SSH without X11.
        #[arg(long)]
        no_browser: bool,
    },
    /// Log out from the TraceVault server
    Logout,
    /// Push commit metadata to the server.
    /// Installed into .git/hooks/post-commit by `tracevault init` and
    /// invoked by git after every commit — not intended to be run manually.
    #[command(hide = true)]
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
    /// Open (or re-open) a validation window for the current session.
    ///
    /// A validation window declares that the agent has finished making changes
    /// and is now running quality checks. Only tool calls made after this point
    /// are evaluated by validation_window-scoped policies. Calling this again
    /// resets the window, discarding earlier window events.
    #[command(name = "validation-start")]
    ValidationStart,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli {
        Cli::Init {
            server_url,
            claude_settings,
            no_gitignore,
        } => {
            let cwd = env::current_dir().expect("Cannot determine current directory");
            match commands::init::init_in_directory(
                &cwd,
                server_url.as_deref(),
                claude_settings,
                no_gitignore,
            )
            .await
            {
                Ok(target) => {
                    let entry = target.gitignore_entry();
                    println!("TraceVault initialized in {}", cwd.display());
                    println!("Claude Code hooks installed ({entry})");
                    println!("Git hooks installed (pre-push, post-commit)");
                    println!("Added .tracevault/ and {entry} to .gitignore");
                    println!(
                        "Nothing needs to be committed — all TraceVault files are local only."
                    );
                    println!(
                        "Other contributors run `tracevault init` to set up their own local hooks."
                    );
                }
                Err(e) => eprintln!("Error: {e}"),
            }
        }
        Cli::Status => {
            let cwd = env::current_dir().expect("Cannot determine current directory");
            let code = commands::status::run_status(&cwd).await;
            if code != 0 {
                std::process::exit(code);
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
        Cli::Login {
            server_url,
            no_browser,
        } => {
            if let Err(e) = commands::login::login(&server_url, no_browser).await {
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
        Cli::ValidationStart => {
            let cwd = env::current_dir().expect("Cannot determine current directory");
            if let Err(e) = commands::validation_window::open_validation_window(&cwd).await {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}
