use git2::Repository;
use sqlx::PgPool;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use uuid::Uuid;

/// Total wall-clock budget for a `git clone`. Bounds clones that hang on a
/// stalled network or auth so the repo lands in 'error' instead of being stuck
/// in 'cloning' forever. Kept below the sync-handler staleness threshold so a
/// hung clone resolves itself with a precise error before a retry kicks in.
const CLONE_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone)]
pub struct RepoManager {
    repos_dir: PathBuf,
}

/// Write a deploy key PEM to a temp file and return its path.
fn write_temp_key(deploy_key_pem: &str) -> Result<PathBuf, String> {
    use std::io::Write;
    let dir = std::env::temp_dir().join("tracevault-keys");
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create temp key dir: {e}"))?;
    let path = dir.join(format!("dk-{}", uuid::Uuid::new_v4()));
    let mut file =
        std::fs::File::create(&path).map_err(|e| format!("failed to create temp key file: {e}"))?;
    // SSH requires PEM files to end with a newline
    let normalized = if deploy_key_pem.ends_with('\n') {
        deploy_key_pem.to_string()
    } else {
        format!("{deploy_key_pem}\n")
    };
    file.write_all(normalized.as_bytes())
        .map_err(|e| format!("failed to write temp key: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("failed to set key permissions: {e}"))?;
    }
    Ok(path)
}

fn cleanup_temp_key(path: &Path) {
    std::fs::remove_file(path).ok();
}

/// `GIT_SSH_COMMAND` value for deploy-key auth. The connect/keepalive timeouts
/// bound the common "SSH hangs forever" failure mode (dead network, silent
/// firewall drop) so git fails instead of stalling indefinitely.
fn ssh_command(deploy_key_path: &Path) -> String {
    format!(
        "ssh -i {} -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new \
         -o ConnectTimeout=15 -o ServerAliveInterval=15 -o ServerAliveCountMax=3",
        deploy_key_path.display()
    )
}

/// Run a (blocking) git command with optional deploy key SSH configuration.
fn git_cmd(deploy_key_path: Option<&Path>) -> Command {
    let mut cmd = Command::new("git");
    if let Some(key_path) = deploy_key_path {
        cmd.env("GIT_SSH_COMMAND", ssh_command(key_path));
    }
    cmd
}

/// Clamp a git error message so a pathological stderr can't bloat the DB row.
fn truncate_error(msg: &str) -> String {
    const MAX: usize = 2000;
    if msg.chars().count() <= MAX {
        msg.to_string()
    } else {
        let head: String = msg.chars().take(MAX).collect();
        format!("{head}…")
    }
}

impl RepoManager {
    pub fn new(repos_dir: &str) -> Self {
        std::fs::create_dir_all(repos_dir).ok();
        Self {
            repos_dir: PathBuf::from(repos_dir),
        }
    }

    pub fn repo_path(&self, repo_id: Uuid) -> PathBuf {
        self.repos_dir.join(repo_id.to_string())
    }

    /// Clone a repo as bare. Updates clone_status in DB.
    pub async fn clone_repo(
        &self,
        pool: &PgPool,
        repo_id: Uuid,
        github_url: &str,
        deploy_key_pem: Option<&str>,
    ) -> Result<(), String> {
        let path = self.repo_path(repo_id);

        sqlx::query(
            "UPDATE repos SET clone_status = 'cloning', clone_started_at = now(), clone_error = NULL WHERE id = $1",
        )
        .bind(repo_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        let temp_key = deploy_key_pem.map(write_temp_key).transpose()?;

        // Async process + wall-clock timeout: a hung clone is killed
        // (`kill_on_drop`) rather than leaving the repo stuck in 'cloning'.
        let mut cmd = tokio::process::Command::new("git");
        if let Some(ref kp) = temp_key {
            cmd.env("GIT_SSH_COMMAND", ssh_command(kp));
        }
        cmd.args(["clone", "--bare", github_url])
            .arg(&path)
            .kill_on_drop(true);

        let result = match tokio::time::timeout(CLONE_TIMEOUT, cmd.output()).await {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(e)) => Err(format!("failed to run git clone: {e}")),
            Err(_) => Err(format!(
                "git clone timed out after {}s",
                CLONE_TIMEOUT.as_secs()
            )),
        };

        if let Some(ref kp) = temp_key {
            cleanup_temp_key(kp);
        }

        // Collapse the outcome to an optional failure message.
        let failure = match result {
            Ok(output) if output.status.success() => None,
            Ok(output) => Some(format!(
                "git clone failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )),
            Err(e) => Some(e),
        };

        let Some(msg) = failure else {
            // Success — clear the failure bookkeeping so the retry budget resets.
            sqlx::query("UPDATE repos SET clone_status = 'ready', clone_path = $1, last_fetched_at = now(), clone_error = NULL, clone_failed_at = NULL, clone_retry_count = 0 WHERE id = $2")
                .bind(path.to_string_lossy().to_string())
                .bind(repo_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            return Ok(());
        };

        // Failure — drop the partial clone and record the error. The retry
        // budget (clone_retry_count) is owned by the sweeper, so leave it be.
        std::fs::remove_dir_all(&path).ok();
        sqlx::query(
            "UPDATE repos SET clone_status = 'error', clone_error = $2, clone_failed_at = now() WHERE id = $1",
        )
        .bind(repo_id)
        .bind(truncate_error(&msg))
        .execute(pool)
        .await
        .ok();
        Err(msg)
    }

    /// Fetch latest changes for a bare repo.
    pub fn fetch_repo(&self, repo_id: Uuid, deploy_key_pem: Option<&str>) -> Result<(), String> {
        let path = self.repo_path(repo_id);
        if !path.exists() {
            return Err("bare repo directory does not exist".into());
        }

        let temp_key = deploy_key_pem.map(write_temp_key).transpose()?;

        let output = git_cmd(temp_key.as_deref())
            .args([
                "-C",
                &path.to_string_lossy(),
                "fetch",
                "origin",
                "+refs/heads/*:refs/heads/*",
                "+refs/tags/*:refs/tags/*",
            ])
            .output()
            .map_err(|e| format!("failed to run git fetch: {e}"));

        if let Some(ref kp) = temp_key {
            cleanup_temp_key(kp);
        }

        let output = output?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("git fetch failed: {stderr}"))
        }
    }

    /// Open a bare repo, returning git2::Repository.
    pub fn open_repo(&self, repo_id: Uuid) -> Result<Repository, String> {
        let path = self.repo_path(repo_id);
        Repository::open_bare(&path).map_err(|e| e.to_string())
    }
}
