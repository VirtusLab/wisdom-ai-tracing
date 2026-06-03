//! Recovery and automatic retry of repo clones.
//!
//! A clone runs in a detached in-process task with no persistence of its own,
//! so a server restart (or a hung git) can leave a repo stranded. This module
//! brings such repos back to life from two entry points wired up in `main`:
//!
//! - [`recover_orphaned_on_startup`] — on boot, re-clone repos left mid-clone.
//! - [`retry_failed_clones`] — periodically re-clone failed repos on a backoff.

use sqlx::PgPool;

use crate::api::repos::get_deploy_key;
use crate::extensions::ExtensionRegistry;
use crate::repo::repos::{GitRepoRepo, RetryableClone};
use crate::repo_manager::RepoManager;

/// Re-clone repos orphaned by the previous shutdown.
///
/// No in-process clone task survives a restart, so any row still in 'cloning'
/// at boot is orphaned. [`GitRepoRepo::reset_orphaned_clones`] resets them
/// (un-wedging the row and refreshing the retry budget) and returns them so we
/// re-clone immediately; a failure falls through to [`retry_failed_clones`].
pub async fn recover_orphaned_on_startup(
    pool: &PgPool,
    repo_manager: &RepoManager,
    extensions: &ExtensionRegistry,
) {
    let orphaned = match GitRepoRepo::reset_orphaned_clones(pool).await {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!("Failed to reset orphaned clones: {e}");
            return;
        }
    };

    if orphaned.is_empty() {
        return;
    }

    tracing::warn!("Re-cloning {} repo(s) orphaned by restart", orphaned.len());
    reclone_repos(pool, repo_manager, extensions, orphaned).await;
}

/// Re-clone failed repos that are due for an automatic retry. Eligible repos
/// are claimed atomically (retry counter bumped) by the query, then re-cloned;
/// `clone_repo` resets the counter on success or records a fresh failure.
pub async fn retry_failed_clones(
    pool: &PgPool,
    repo_manager: &RepoManager,
    extensions: &ExtensionRegistry,
) {
    let claimed = match GitRepoRepo::claim_clones_for_retry(pool).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to query clones for auto-retry: {e}");
            return;
        }
    };

    if claimed.is_empty() {
        return;
    }

    tracing::info!("Auto-retrying {} failed clone(s)...", claimed.len());
    reclone_repos(pool, repo_manager, extensions, claimed).await;
}

/// Re-clone a set of repos sequentially. Decrypts each deploy key (logging and
/// skipping on failure) and drives `clone_repo`, which records the outcome in
/// the DB.
async fn reclone_repos(
    pool: &PgPool,
    repo_manager: &RepoManager,
    extensions: &ExtensionRegistry,
    repos: Vec<RetryableClone>,
) {
    for repo in repos {
        let Some(url) = repo.github_url else { continue };
        let deploy_key: Option<String> = if repo.deploy_key_encrypted.is_some() {
            match get_deploy_key(pool, repo.id, extensions.encryption.as_ref()).await {
                Ok(key) => key,
                Err(e) => {
                    tracing::warn!("Failed to decrypt deploy key for repo {}: {e}", repo.id);
                    None
                }
            }
        } else {
            None
        };

        if let Err(e) = repo_manager
            .clone_repo(pool, repo.id, &url, deploy_key.as_deref())
            .await
        {
            tracing::warn!("Clone failed for repo {}: {e}", repo.id);
        }
    }
}
