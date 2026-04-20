use crate::repo_manager::RepoManager;
use sqlx::PgPool;
use tracevault_core::code_nav::CodeScope;
use uuid::Uuid;

#[derive(Debug, serde::Serialize)]
pub struct SessionCommit {
    pub commit_sha: String,
    pub message: String,
    pub session_ids: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct StoryContext {
    pub function_source: String,
    pub changes: Vec<ChangeEntry>,
    /// Commits with linked sessions found via file_path DB query.
    /// These may have different SHAs than the git-log changes (due to rebases).
    pub session_commits: Vec<SessionCommit>,
}

#[derive(Debug, serde::Serialize)]
pub struct SessionRef {
    pub id: Uuid,
    pub session_id: String,
    pub model: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct FunctionSessions {
    pub function_name: String,
    pub line_range: (usize, usize),
    pub sessions: Vec<FunctionSessionRef>,
}

#[derive(Debug, serde::Serialize)]
pub struct FunctionSessionRef {
    pub id: Uuid,
    pub session_id: String,
    pub model: Option<String>,
    pub user_email: Option<String>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub commit_shas: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct ChangeEntry {
    pub commit_sha: String,
    pub commit_uuid: Option<Uuid>,
    pub author: String,
    pub date: String,
    pub message: String,
    pub diff_excerpt: Option<String>,
    pub session_transcript_excerpt: Option<String>,
    pub ai_percentage: Option<f64>,
    pub model: Option<String>,
    pub sessions: Vec<SessionRef>,
}

/// Intermediate struct for git data extracted synchronously
struct GitCommitInfo {
    sha: String,
    author: String,
    date: String,
    message: String,
}

/// Walk git log and return SHAs of commits that touched a given file.
/// Returns up to `limit` SHAs in chronological order (oldest first).
pub fn collect_file_commit_shas(
    repo_manager: &RepoManager,
    repo_id: Uuid,
    git_ref: &str,
    file_path: &str,
    limit: usize,
) -> Result<Vec<String>, String> {
    let repo = repo_manager.open_repo(repo_id)?;

    let obj = repo.revparse_single(git_ref).map_err(|e| e.to_string())?;
    let commit = obj.peel_to_commit().map_err(|e| e.to_string())?;

    let mut revwalk = repo.revwalk().map_err(|e| e.to_string())?;
    revwalk.push(commit.id()).map_err(|e| e.to_string())?;
    revwalk
        .set_sorting(git2::Sort::TIME | git2::Sort::REVERSE)
        .map_err(|e| e.to_string())?;

    let mut shas = Vec::new();
    for oid in revwalk.take(limit) {
        let oid = oid.map_err(|e| e.to_string())?;
        let c = repo.find_commit(oid).map_err(|e| e.to_string())?;

        let touches_file = c
            .parent(0)
            .ok()
            .and_then(|parent| {
                let old_tree = parent.tree().ok()?;
                let new_tree = c.tree().ok()?;
                let diff = repo
                    .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)
                    .ok()?;
                Some(diff.deltas().any(|d| {
                    d.new_file()
                        .path()
                        .map(|p| p.to_string_lossy() == file_path)
                        .unwrap_or(false)
                }))
            })
            .unwrap_or(true);

        if !touches_file {
            continue;
        }

        shas.push(oid.to_string());
    }

    Ok(shas)
}

pub async fn gather_story_context(
    repo_manager: &RepoManager,
    pool: &PgPool,
    repo_id: Uuid,
    git_ref: &str,
    file_path: &str,
    scope: &CodeScope,
) -> Result<StoryContext, String> {
    // 1a. Extract function source (git2 types are !Send, scoped block)
    let function_source = {
        let repo = repo_manager.open_repo(repo_id)?;
        let obj = repo.revparse_single(git_ref).map_err(|e| e.to_string())?;
        let commit = obj.peel_to_commit().map_err(|e| e.to_string())?;
        let tree = commit.tree().map_err(|e| e.to_string())?;
        let entry = tree
            .get_path(std::path::Path::new(file_path))
            .map_err(|e| e.to_string())?;
        let blob = entry.to_object(&repo).map_err(|e| e.to_string())?;
        let content = blob.as_blob().ok_or("Not a blob")?;
        let source = String::from_utf8_lossy(content.content()).to_string();
        let lines: Vec<&str> = source.lines().collect();
        let func_lines =
            &lines[scope.start_line.saturating_sub(1)..scope.end_line.min(lines.len())];
        func_lines.join("\n")
    };

    // 1b. Collect commit SHAs touching this file
    let commit_shas = collect_file_commit_shas(repo_manager, repo_id, git_ref, file_path, 200)?;

    // 1c. Extract commit metadata (git2 types are !Send, scoped block)
    let git_commits: Vec<GitCommitInfo> = {
        let repo = repo_manager.open_repo(repo_id)?;
        commit_shas
            .iter()
            .filter_map(|sha| {
                let oid = git2::Oid::from_str(sha).ok()?;
                let c = repo.find_commit(oid).ok()?;
                let info = GitCommitInfo {
                    sha: sha.clone(),
                    author: c.author().name().unwrap_or("unknown").to_string(),
                    date: c.time().seconds().to_string(),
                    message: c.message().unwrap_or("").trim().to_string(),
                };
                Some(info)
            })
            .collect()
    };
    // All git2 objects are now dropped — safe to .await

    // 2. Enrich with TraceVault DB data (async)
    println!(
        "gather_story_context: enriching {} commits for repo_id={}",
        git_commits.len(),
        repo_id
    );
    let mut changes = Vec::new();
    for gc in &git_commits {
        // Fetch commit UUID and attribution
        let tv_data = sqlx::query_as::<_, (Uuid, Option<serde_json::Value>)>(
            "SELECT c.id, c.diff_data \
             FROM commits c \
             WHERE c.repo_id = $1 AND c.commit_sha = $2 LIMIT 1",
        )
        .bind(repo_id)
        .bind(&gc.sha)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

        let (commit_uuid, ai_percentage, session_excerpt, model, sessions) =
            if let Some((c_id, _diff_data)) = &tv_data {
                // Fetch sessions linked via commit_attributions
                let session_rows = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
                    "SELECT DISTINCT s.id, s.session_id, s.model \
                     FROM sessions s \
                     JOIN commit_attributions ca ON ca.session_id = s.id \
                     WHERE ca.commit_id = $1 \
                     ORDER BY s.id",
                )
                .bind(c_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                println!(
                    "  commit {} -> c_id={}, sessions={}",
                    &gc.sha[..8],
                    c_id,
                    session_rows.len()
                );

                let model = session_rows.first().and_then(|r| r.2.clone());

                // Get transcript excerpts from sessions (newest first, budget-limited)
                let excerpt = {
                    let mut combined_excerpts = Vec::new();
                    let mut total_chars = 0usize;
                    const TRANSCRIPT_BUDGET: usize = 40_000;
                    const PER_SESSION_LIMIT: usize = 8_000;

                    // Iterate sessions in reverse (newest first)
                    for (sid, session_id, _) in session_rows.iter().rev() {
                        if total_chars >= TRANSCRIPT_BUDGET {
                            break;
                        }

                        let chunks: Vec<(serde_json::Value,)> = sqlx::query_as(
                            "SELECT data FROM transcript_chunks \
                             WHERE session_id = $1 \
                             ORDER BY chunk_index ASC",
                        )
                        .bind(sid)
                        .fetch_all(pool)
                        .await
                        .unwrap_or_default();

                        if chunks.is_empty() {
                            continue;
                        }

                        let transcript_array: Vec<serde_json::Value> =
                            chunks.into_iter().map(|(d,)| d).collect();
                        let transcript_val = serde_json::Value::Array(transcript_array);
                        if let Some(excerpt) = extract_relevant_excerpt(
                            &transcript_val,
                            file_path,
                            &scope.name,
                            PER_SESSION_LIMIT,
                        ) {
                            let remaining = TRANSCRIPT_BUDGET - total_chars;
                            let truncated: String = excerpt.chars().take(remaining).collect();
                            total_chars += truncated.len();
                            combined_excerpts.push(format!(
                                "[Session {}]:\n{}",
                                &session_id[..8.min(session_id.len())],
                                truncated
                            ));
                        }
                    }

                    if combined_excerpts.is_empty() {
                        None
                    } else {
                        Some(combined_excerpts.join("\n\n"))
                    }
                };

                // Compute ai_percentage from attributions
                let pct: Option<f64> = sqlx::query_scalar(
                    "SELECT AVG(ca.confidence)::float8 * 100.0 \
                     FROM commit_attributions ca \
                     WHERE ca.commit_id = $1",
                )
                .bind(c_id)
                .fetch_one(pool)
                .await
                .ok();

                let sessions: Vec<SessionRef> = session_rows
                    .into_iter()
                    .map(|(id, sid, m)| SessionRef {
                        id,
                        session_id: sid,
                        model: m,
                    })
                    .collect();

                (Some(*c_id), pct, excerpt, model, sessions)
            } else {
                (None, None, None, None, vec![])
            };

        changes.push(ChangeEntry {
            commit_sha: gc.sha.clone(),
            commit_uuid,
            author: gc.author.clone(),
            date: gc.date.clone(),
            message: gc.message.clone(),
            diff_excerpt: None,
            session_transcript_excerpt: session_excerpt,
            ai_percentage,
            model,
            sessions,
        });
    }

    // 3. Query session-linked commits directly by file_path from the DB.
    // Git-log SHAs may differ from DB SHAs (rebases/squashes), so this
    // provides the authoritative session-to-commit mapping for the prompt.
    let db_rows = sqlx::query_as::<_, (String, String, String, Option<String>)>(
        "SELECT c.commit_sha, COALESCE(c.message, ''), s.session_id, s.model \
         FROM commits c \
         JOIN commit_attributions ca ON ca.commit_id = c.id \
         JOIN sessions s ON s.id = ca.session_id \
         WHERE c.repo_id = $1 AND ca.file_path = $2 \
         ORDER BY s.started_at DESC NULLS LAST",
    )
    .bind(repo_id)
    .bind(file_path)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // Group into SessionCommit entries
    let session_commits = {
        use std::collections::HashMap;
        let mut map: HashMap<String, SessionCommit> = HashMap::new();
        for (sha, msg, sid, _model) in &db_rows {
            map.entry(sha.clone())
                .and_modify(|sc| {
                    if !sc.session_ids.contains(sid) {
                        sc.session_ids.push(sid.clone());
                    }
                })
                .or_insert_with(|| SessionCommit {
                    commit_sha: sha.clone(),
                    message: msg.clone(),
                    session_ids: vec![sid.clone()],
                });
        }
        map.into_values().collect::<Vec<_>>()
    };

    // Also attach transcripts to the first change entry if none found via SHA matching
    let has_sessions = changes.iter().any(|c| !c.sessions.is_empty());
    if !has_sessions && !db_rows.is_empty() {
        // Collect unique session IDs for transcript fetching
        let mut seen_sids = std::collections::HashSet::new();
        let mut session_refs = Vec::new();
        for (_, _, sid, model) in &db_rows {
            if seen_sids.insert(sid.clone()) {
                // Look up session UUID
                if let Some((s_uuid,)) = sqlx::query_as::<_, (Uuid,)>(
                    "SELECT id FROM sessions WHERE session_id = $1 LIMIT 1",
                )
                .bind(sid)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten()
                {
                    session_refs.push(SessionRef {
                        id: s_uuid,
                        session_id: sid.clone(),
                        model: model.clone(),
                    });
                }
            }
        }

        // Fetch transcripts
        let mut combined_excerpts = Vec::new();
        let mut total_chars = 0usize;
        const FALLBACK_BUDGET: usize = 40_000;
        const FALLBACK_PER_SESSION: usize = 8_000;

        for sr in &session_refs {
            if total_chars >= FALLBACK_BUDGET {
                break;
            }
            let chunks: Vec<(serde_json::Value,)> = sqlx::query_as(
                "SELECT data FROM transcript_chunks \
                 WHERE session_id = $1 \
                 ORDER BY chunk_index ASC",
            )
            .bind(sr.id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            if chunks.is_empty() {
                continue;
            }
            let transcript_array: Vec<serde_json::Value> =
                chunks.into_iter().map(|(d,)| d).collect();
            let transcript_val = serde_json::Value::Array(transcript_array);
            if let Some(excerpt) = extract_relevant_excerpt(
                &transcript_val,
                file_path,
                &scope.name,
                FALLBACK_PER_SESSION,
            ) {
                let remaining = FALLBACK_BUDGET - total_chars;
                let truncated: String = excerpt.chars().take(remaining).collect();
                total_chars += truncated.len();
                combined_excerpts.push(format!(
                    "[Session {}]:\n{}",
                    &sr.session_id[..8.min(sr.session_id.len())],
                    truncated
                ));
            }
        }

        let model = session_refs.first().and_then(|s| s.model.clone());
        let excerpt = if combined_excerpts.is_empty() {
            None
        } else {
            Some(combined_excerpts.join("\n\n"))
        };

        if let Some(first_change) = changes.first_mut() {
            first_change.sessions = session_refs;
            first_change.model = model;
            first_change.session_transcript_excerpt = excerpt;
        }
    }

    Ok(StoryContext {
        function_source,
        changes,
        session_commits,
    })
}

pub async fn gather_function_sessions(
    _repo_manager: &RepoManager,
    pool: &PgPool,
    repo_id: Uuid,
    _git_ref: &str,
    file_path: &str,
    scope: &CodeScope,
) -> Result<FunctionSessions, String> {
    // Query sessions directly from DB via commit_attributions.file_path
    // This is more reliable than git-walking because commit SHAs in the DB
    // may differ from git history (rebases, squashes, etc.)
    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            Option<String>,
            Option<String>,
            Option<chrono::DateTime<chrono::Utc>>,
            String,
        ),
    >(
        "SELECT DISTINCT s.id, s.session_id, s.model, u.email, s.started_at, c.commit_sha \
         FROM sessions s \
         JOIN commit_attributions ca ON ca.session_id = s.id \
         JOIN commits c ON c.id = ca.commit_id \
         LEFT JOIN users u ON u.id = s.user_id \
         WHERE c.repo_id = $1 AND ca.file_path = $2 \
         ORDER BY s.started_at DESC NULLS LAST",
    )
    .bind(repo_id)
    .bind(file_path)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    // Deduplicate sessions, accumulating commit SHAs
    use std::collections::HashMap;
    let mut session_map: HashMap<Uuid, FunctionSessionRef> = HashMap::new();

    for (sid, session_id, model, email, started_at, commit_sha) in rows {
        session_map
            .entry(sid)
            .and_modify(|entry| {
                if !entry.commit_shas.contains(&commit_sha) {
                    entry.commit_shas.push(commit_sha.clone());
                }
            })
            .or_insert(FunctionSessionRef {
                id: sid,
                session_id,
                model,
                user_email: email,
                started_at,
                commit_shas: vec![commit_sha],
            });
    }

    let mut sessions: Vec<FunctionSessionRef> = session_map.into_values().collect();
    sessions.sort_by_key(|s| std::cmp::Reverse(s.started_at));

    Ok(FunctionSessions {
        function_name: scope.name.clone(),
        line_range: (scope.start_line, scope.end_line),
        sessions,
    })
}

fn extract_relevant_excerpt(
    transcript: &serde_json::Value,
    file_path: &str,
    func_name: &str,
    max_chars: usize,
) -> Option<String> {
    let text = serde_json::to_string(transcript).ok()?;
    let mut excerpts = Vec::new();
    for line in text.lines() {
        if line.contains(file_path) || line.contains(func_name) {
            excerpts.push(line.to_string());
        }
    }
    if excerpts.is_empty() {
        return None;
    }
    Some(excerpts.join("\n").chars().take(max_chars).collect())
}

pub fn build_story_prompt(ctx: &StoryContext) -> String {
    let mut prompt = String::from(
        "You are documenting the history of a code function for a development team.\n\n\
         Write a clear, concise narrative. Focus on WHY decisions were made, not just WHAT changed.\n\n\
         ## Current Code\n```\n",
    );
    prompt.push_str(&ctx.function_source);
    prompt.push_str("\n```\n\n## Change History (chronological)\n\n");

    for (i, change) in ctx.changes.iter().enumerate() {
        prompt.push_str(&format!(
            "### Change {}: {} by {} on {}\n",
            i + 1,
            change.commit_sha,
            change.author,
            change.date
        ));
        prompt.push_str(&format!("Commit message: {}\n", change.message));
        if !change.sessions.is_empty() {
            let session_ids: Vec<String> = change
                .sessions
                .iter()
                .map(|s| {
                    let short = &s.id.to_string()[..8];
                    format!(
                        "{} (model: {})",
                        short,
                        s.model.as_deref().unwrap_or("unknown")
                    )
                })
                .collect();
            prompt.push_str(&format!("Sessions: {}\n", session_ids.join(", ")));
        }
        if let Some(model) = &change.model {
            prompt.push_str(&format!("AI Model: {}\n", model));
        }
        if let Some(pct) = change.ai_percentage {
            prompt.push_str(&format!("Attribution: {:.0}% AI-generated\n", pct));
        }
        if let Some(excerpt) = &change.session_transcript_excerpt {
            prompt.push_str(&format!("AI Session Transcripts:\n{}\n", excerpt));
        }
        prompt.push('\n');
    }

    // Add session-linked commits with their session IDs
    if !ctx.session_commits.is_empty() {
        prompt.push_str("## AI Sessions and Commits\n\n");
        prompt.push_str(
            "These commits were made during AI coding sessions that modified this file.\n\
             Each commit lists the session IDs that contributed to it.\n\n",
        );
        for sc in &ctx.session_commits {
            let session_list = sc
                .session_ids
                .iter()
                .map(|s| format!("`{}`", &s[..8.min(s.len())]))
                .collect::<Vec<_>>()
                .join(", ");
            prompt.push_str(&format!(
                "- Commit `{}` — {} (sessions: {})\n",
                &sc.commit_sha[..7.min(sc.commit_sha.len())],
                sc.message.lines().next().unwrap_or(""),
                session_list
            ));
        }
        prompt.push('\n');
    }

    prompt.push_str(
        "## Task\nWrite a clear narrative explaining:\n\
         1. Why this code exists (original intent)\n\
         2. How it evolved over time\n\
         3. Key decisions made (especially insights from AI session transcripts)\n\
         4. Current state and any notable patterns\n\n\
         Keep it concise but informative. Use markdown formatting.\n\
         IMPORTANT: When referencing changes, use the short commit SHAs (e.g., `abc1234`) and \
         session IDs (e.g., `a1b2c3d4`) from the 'AI Sessions and Commits' section. \
         Both will become clickable links in the UI. Reference specific sessions when \
         discussing insights from their transcripts.",
    );

    if prompt.len() > 100_000 {
        prompt.truncate(100_000);
        prompt.push_str("\n\n[Context truncated due to length]");
    }

    prompt
}
