use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::{extractors::OrgAuth, permissions::Permission, AppState};

// --- Helpers ---

async fn verify_repo_access(state: &AppState, org_id: Uuid, repo_id: Uuid) -> Result<(), AppError> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM repos WHERE id = $1 AND org_id = $2 AND clone_status = 'ready')",
    )
    .bind(repo_id)
    .bind(org_id)
    .fetch_one(&state.pool)
    .await?;

    if !exists {
        return Err(AppError::NotFound("Repo not found or not ready".into()));
    }
    Ok(())
}

fn detect_language(path: &str) -> Option<String> {
    let ext = path.rsplit('.').next()?;
    match ext {
        "rs" => Some("rust"),
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" => Some("javascript"),
        "py" => Some("python"),
        "go" => Some("go"),
        "java" => Some("java"),
        "scala" | "sc" => Some("scala"),
        "json" => Some("json"),
        "toml" => Some("toml"),
        "yaml" | "yml" => Some("yaml"),
        "md" => Some("markdown"),
        "sql" => Some("sql"),
        "html" => Some("html"),
        "css" => Some("css"),
        "svelte" => Some("svelte"),
        "sh" | "bash" => Some("bash"),
        _ => None,
    }
    .map(String::from)
}

fn default_ref() -> String {
    "HEAD".to_string()
}

// --- Types ---

#[derive(Serialize)]
pub struct BranchInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub ref_type: String,
    pub is_default: bool,
}

#[derive(Deserialize)]
pub struct TreeQuery {
    #[serde(rename = "ref", default = "default_ref")]
    pub git_ref: String,
    #[serde(default)]
    pub path: String,
}

#[derive(Serialize)]
pub struct TreeEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub size: Option<u64>,
    pub path: String,
}

#[derive(Deserialize)]
pub struct BlobQuery {
    #[serde(rename = "ref", default = "default_ref")]
    pub git_ref: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct BlobResponse {
    pub content: Option<String>,
    pub size: u64,
    pub language: Option<String>,
    pub truncated: bool,
    pub path: String,
}

#[derive(Serialize)]
pub struct BlameHunk {
    pub commit_sha: String,
    pub author: String,
    pub date: String,
    pub start_line: usize,
    pub end_line: usize,
    pub message: Option<String>,
}

#[derive(Serialize)]
pub struct FileCommit {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub date: String,
}

#[derive(Deserialize)]
pub struct RefQuery {
    #[serde(rename = "ref", default = "default_ref")]
    pub git_ref: String,
}

#[derive(Serialize)]
pub struct RefInfo {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub date: String,
}

#[derive(Deserialize)]
pub struct StoryRequest {
    pub path: String,
    pub line: usize,
    #[serde(rename = "ref", default = "default_ref")]
    pub git_ref: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CommitRef {
    pub sha: String,
    pub id: Option<String>,
    pub message: String,
    pub author: String,
    pub sessions: Vec<SessionRefResponse>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SessionRefResponse {
    pub id: String,
    pub session_id: String,
    pub model: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct StoryResponse {
    pub story: String,
    pub function_name: String,
    pub kind: String,
    pub line_range: [usize; 2],
    pub commits_analyzed: Vec<String>,
    pub sessions_referenced: Vec<String>,
    pub references: Vec<CommitRef>,
    pub cached: bool,
    pub generated_at: String,
}

#[derive(Deserialize)]
pub struct FunctionSessionsQuery {
    #[serde(rename = "ref", default = "default_ref")]
    pub git_ref: String,
    pub path: String,
    pub line: usize,
}

#[derive(Serialize)]
pub struct FunctionSessionRefResponse {
    pub id: String,
    pub session_id: String,
    pub model: Option<String>,
    pub user_email: Option<String>,
    pub started_at: Option<String>,
    pub commit_shas: Vec<String>,
}

#[derive(Serialize)]
pub struct FunctionSessionsResponse {
    pub function_name: String,
    pub line_range: [usize; 2],
    pub sessions: Vec<FunctionSessionRefResponse>,
}

// --- Endpoints ---

pub async fn list_branches(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, repo_id)): Path<(String, Uuid)>,
) -> Result<Json<Vec<BranchInfo>>, AppError> {
    if !state
        .extensions
        .permissions
        .has_permission(&auth.role, Permission::CodeBrowse)
    {
        return Err(AppError::Forbidden("Insufficient permissions".into()));
    }
    verify_repo_access(&state, auth.org_id, repo_id).await?;

    let repo = state
        .repo_manager
        .open_repo(repo_id)
        .map_err(AppError::internal)?;

    let mut branches = Vec::new();
    let head_ref = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(String::from));

    for branch in repo.branches(None)? {
        let (branch, _branch_type) = branch?;
        if let Some(name) = branch.name().ok().flatten() {
            let is_default = head_ref.as_deref() == Some(name);
            branches.push(BranchInfo {
                name: name.to_string(),
                ref_type: "branch".to_string(),
                is_default,
            });
        }
    }

    if let Ok(tags) = repo.tag_names(None) {
        for i in 0..tags.len() {
            if let Some(name) = tags.get(i) {
                branches.push(BranchInfo {
                    name: name.to_string(),
                    ref_type: "tag".to_string(),
                    is_default: false,
                });
            }
        }
    }

    Ok(Json(branches))
}

pub async fn get_tree(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, repo_id)): Path<(String, Uuid)>,
    Query(query): Query<TreeQuery>,
) -> Result<Json<Vec<TreeEntry>>, AppError> {
    if !state
        .extensions
        .permissions
        .has_permission(&auth.role, Permission::CodeBrowse)
    {
        return Err(AppError::Forbidden("Insufficient permissions".into()));
    }
    verify_repo_access(&state, auth.org_id, repo_id).await?;

    let repo = state
        .repo_manager
        .open_repo(repo_id)
        .map_err(AppError::internal)?;

    let obj = repo
        .revparse_single(&query.git_ref)
        .map_err(|e| AppError::BadRequest(format!("Invalid ref: {e}")))?;
    let commit = obj
        .peel_to_commit()
        .map_err(|e| AppError::BadRequest(format!("Ref is not a commit: {e}")))?;
    let tree = commit.tree()?;

    let target_tree = if query.path.is_empty() || query.path == "/" {
        tree
    } else {
        let entry = tree
            .get_path(std::path::Path::new(&query.path))
            .map_err(|e| AppError::NotFound(format!("Path not found: {e}")))?;
        let obj = entry.to_object(&repo)?;
        obj.into_tree()
            .map_err(|_| AppError::BadRequest("Path is not a directory".into()))?
    };

    let mut entries = Vec::new();
    for entry in target_tree.iter() {
        let name = entry.name().unwrap_or("").to_string();
        let entry_type = match entry.kind() {
            Some(git2::ObjectType::Tree) => "tree",
            Some(git2::ObjectType::Blob) => "blob",
            _ => continue,
        };
        let size = if entry_type == "blob" {
            entry
                .to_object(&repo)
                .ok()
                .and_then(|o| o.as_blob().map(|b| b.size() as u64))
        } else {
            None
        };
        let path = if query.path.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", query.path.trim_end_matches('/'), name)
        };
        entries.push(TreeEntry {
            name,
            entry_type: entry_type.to_string(),
            size,
            path,
        });
    }

    entries.sort_by(|a, b| {
        let type_order = |t: &str| if t == "tree" { 0 } else { 1 };
        type_order(&a.entry_type)
            .cmp(&type_order(&b.entry_type))
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(Json(entries))
}

pub async fn get_blob(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, repo_id)): Path<(String, Uuid)>,
    Query(query): Query<BlobQuery>,
) -> Result<Json<BlobResponse>, AppError> {
    if !state
        .extensions
        .permissions
        .has_permission(&auth.role, Permission::CodeBrowse)
    {
        return Err(AppError::Forbidden("Insufficient permissions".into()));
    }
    verify_repo_access(&state, auth.org_id, repo_id).await?;

    let repo = state
        .repo_manager
        .open_repo(repo_id)
        .map_err(AppError::internal)?;

    let obj = repo
        .revparse_single(&query.git_ref)
        .map_err(|e| AppError::BadRequest(format!("Invalid ref: {e}")))?;
    let commit = obj
        .peel_to_commit()
        .map_err(|e| AppError::BadRequest(format!("Ref is not a commit: {e}")))?;
    let tree = commit.tree()?;

    // Prevent path traversal attacks by rejecting paths containing '..'.
    let path = std::path::Path::new(&query.path);
    if path
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return Err(AppError::BadRequest(format!(
            "Invalid input: {}",
            path.display()
        )));
    }

    let entry = tree
        .get_path(path)
        .map_err(|e| AppError::NotFound(format!("File not found: {e}")))?;
    let blob_obj = entry.to_object(&repo)?;
    let blob = blob_obj
        .as_blob()
        .ok_or_else(|| AppError::BadRequest("Path is not a file".into()))?;

    let size = blob.size() as u64;
    let max_size = 1_048_576; // 1MB

    let (content, truncated) = if size > max_size {
        (None, true)
    } else if blob.is_binary() {
        (None, false)
    } else {
        (
            Some(String::from_utf8_lossy(blob.content()).to_string()),
            false,
        )
    };

    let language = detect_language(&query.path);

    Ok(Json(BlobResponse {
        content,
        size,
        language,
        truncated,
        path: query.path,
    }))
}

pub async fn get_blame(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, repo_id)): Path<(String, Uuid)>,
    Query(query): Query<BlobQuery>,
) -> Result<Json<Vec<BlameHunk>>, AppError> {
    if !state
        .extensions
        .permissions
        .has_permission(&auth.role, Permission::CodeBrowse)
    {
        return Err(AppError::Forbidden("Insufficient permissions".into()));
    }
    verify_repo_access(&state, auth.org_id, repo_id).await?;

    let repo = state
        .repo_manager
        .open_repo(repo_id)
        .map_err(AppError::internal)?;

    let obj = repo
        .revparse_single(&query.git_ref)
        .map_err(|e| AppError::BadRequest(format!("Invalid ref: {e}")))?;
    let commit = obj
        .peel_to_commit()
        .map_err(|e| AppError::BadRequest(format!("Ref is not a commit: {e}")))?;

    let blame = repo
        .blame_file(
            std::path::Path::new(&query.path),
            Some(git2::BlameOptions::new().newest_commit(commit.id())),
        )
        .map_err(|e| AppError::Internal(format!("Blame failed: {e}")))?;

    let mut hunks = Vec::new();
    for i in 0..blame.len() {
        let hunk = blame.get_index(i).unwrap();
        let sig = hunk.final_signature();
        let commit_id = hunk.final_commit_id();
        let message = repo
            .find_commit(commit_id)
            .ok()
            .and_then(|c| c.message().map(|m| m.trim().to_string()));

        hunks.push(BlameHunk {
            commit_sha: commit_id.to_string(),
            author: sig.name().unwrap_or("unknown").to_string(),
            date: sig.when().seconds().to_string(),
            start_line: hunk.final_start_line(),
            end_line: hunk.final_start_line() + hunk.lines_in_hunk() - 1,
            message,
        });
    }

    Ok(Json(hunks))
}

pub async fn list_file_commits(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, repo_id)): Path<(String, Uuid)>,
    Query(query): Query<BlobQuery>,
) -> Result<Json<Vec<FileCommit>>, AppError> {
    if !state
        .extensions
        .permissions
        .has_permission(&auth.role, Permission::CodeBrowse)
    {
        return Err(AppError::Forbidden("Insufficient permissions".into()));
    }
    verify_repo_access(&state, auth.org_id, repo_id).await?;

    let repo = state
        .repo_manager
        .open_repo(repo_id)
        .map_err(AppError::internal)?;

    let obj = repo
        .revparse_single(&query.git_ref)
        .map_err(|e| AppError::BadRequest(format!("Invalid ref: {e}")))?;
    let commit = obj
        .peel_to_commit()
        .map_err(|e| AppError::BadRequest(format!("Ref is not a commit: {e}")))?;

    let mut revwalk = repo.revwalk()?;
    revwalk.push(commit.id())?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    let mut commits = Vec::new();
    for oid in revwalk.take(100) {
        let oid = oid?;
        let c = repo.find_commit(oid)?;

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
                        .map(|p| p.to_string_lossy() == query.path)
                        .unwrap_or(false)
                        || d.old_file()
                            .path()
                            .map(|p| p.to_string_lossy() == query.path)
                            .unwrap_or(false)
                }))
            })
            .unwrap_or(true); // Include first commit (no parent)

        if touches_file {
            commits.push(FileCommit {
                sha: oid.to_string(),
                message: c.message().unwrap_or("").trim().to_string(),
                author: c.author().name().unwrap_or("unknown").to_string(),
                date: c.time().seconds().to_string(),
            });
        }
    }

    Ok(Json(commits))
}

pub async fn get_ref_info(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, repo_id)): Path<(String, Uuid)>,
    Query(query): Query<RefQuery>,
) -> Result<Json<RefInfo>, AppError> {
    if !state
        .extensions
        .permissions
        .has_permission(&auth.role, Permission::CodeBrowse)
    {
        return Err(AppError::Forbidden("Insufficient permissions".into()));
    }
    verify_repo_access(&state, auth.org_id, repo_id).await?;

    let repo = state
        .repo_manager
        .open_repo(repo_id)
        .map_err(AppError::internal)?;

    let obj = repo
        .revparse_single(&query.git_ref)
        .map_err(|e| AppError::BadRequest(format!("Invalid ref: {e}")))?;
    let commit = obj
        .peel_to_commit()
        .map_err(|e| AppError::BadRequest(format!("Ref is not a commit: {e}")))?;

    let info = RefInfo {
        sha: commit.id().to_string(),
        message: commit.message().unwrap_or("").trim().to_string(),
        author: commit.author().name().unwrap_or("unknown").to_string(),
        date: commit.time().seconds().to_string(),
    };
    Ok(Json(info))
}

// --- Story endpoint ---

pub async fn generate_story(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, repo_id)): Path<(String, Uuid)>,
    Json(req): Json<StoryRequest>,
) -> Result<Json<StoryResponse>, AppError> {
    if !state
        .extensions
        .permissions
        .has_permission(&auth.role, Permission::CodeBrowse)
    {
        return Err(AppError::Forbidden("Insufficient permissions".into()));
    }

    let force = req.force;

    verify_repo_access(&state, auth.org_id, repo_id).await?;

    // All git2 operations in a scoped block — git2 types are !Send
    // so they must be dropped before any .await
    let (scope, head_sha) = {
        let repo = state
            .repo_manager
            .open_repo(repo_id)
            .map_err(AppError::internal)?;

        let ext = req.path.rsplit('.').next().unwrap_or("");

        let obj = repo
            .revparse_single(&req.git_ref)
            .map_err(|e| AppError::BadRequest(format!("Invalid ref: {e}")))?;
        let commit = obj
            .peel_to_commit()
            .map_err(|e| AppError::BadRequest(format!("Ref is not a commit: {e}")))?;
        let tree = commit.tree()?;
        let entry = tree
            .get_path(std::path::Path::new(&req.path))
            .map_err(|e| AppError::NotFound(format!("File not found: {e}")))?;
        let blob_obj = entry.to_object(&repo)?;
        let blob = blob_obj
            .as_blob()
            .ok_or_else(|| AppError::BadRequest("Path is not a file".into()))?;
        let source = String::from_utf8_lossy(blob.content()).to_string();

        let scope = tracevault_core::code_nav::find_enclosing_scope(&source, ext, req.line)
            .unwrap_or_else(|| tracevault_core::code_nav::fallback_scope(&source, req.line, 20));

        let head_sha = commit.id().to_string();
        (scope, head_sha)
    };

    // Check cache first — users with StoryView (e.g. auditors) can read cached
    // stories but cannot trigger new LLM generation
    if !force {
        if let Some(cached) = check_story_cache(
            &state.pool,
            repo_id,
            &req.path,
            &scope.name,
            &req.git_ref,
            &head_sha,
        )
        .await
        {
            if state
                .extensions
                .permissions
                .has_permission(&auth.role, Permission::StoryView)
            {
                return Ok(Json(cached));
            }
        }
    }

    // New story generation requires StoryGenerate permission
    if !state
        .extensions
        .permissions
        .has_permission(&auth.role, Permission::StoryGenerate)
    {
        return Err(AppError::Forbidden(
            "Insufficient permissions to generate stories".into(),
        ));
    }

    // Resolve per-org LLM — this is the only path for story generation
    let org_llm = resolve_org_llm(&state, auth.org_id).await;
    let org_story: Option<crate::extensions::LlmStoryProvider> =
        org_llm.map(|llm| crate::extensions::LlmStoryProvider::new(std::sync::Arc::from(llm)));
    let story_provider: &dyn crate::extensions::StoryProvider = if let Some(ref org) = org_story {
        org
    } else {
        return Err(AppError::Internal(
            "No LLM configured for this organization".into(),
        ));
    };

    // gather_story_context also uses git2, but it manages its own repo lifetime
    let ctx = crate::story::gather_story_context(
        &state.repo_manager,
        &state.pool,
        repo_id,
        &req.git_ref,
        &req.path,
        &scope,
    )
    .await
    .map_err(AppError::internal)?;

    let prompt = crate::story::build_story_prompt(&ctx);
    let story_md = story_provider
        .generate_story(&prompt, 4096)
        .await
        .map_err(|e| AppError::Internal(format!("LLM error: {e}")))?;

    // Build structured references from context
    let mut references: Vec<CommitRef> = ctx
        .changes
        .iter()
        .map(|c| CommitRef {
            sha: c.commit_sha.clone(),
            id: c.commit_uuid.map(|u| u.to_string()),
            message: c.message.clone(),
            author: c.author.clone(),
            sessions: c
                .sessions
                .iter()
                .map(|s| SessionRefResponse {
                    id: s.id.to_string(),
                    session_id: s.session_id.clone(),
                    model: s.model.clone(),
                })
                .collect(),
        })
        .collect();

    // Supplement references with session-linked commits from DB
    // (handles cases where git SHAs don't match DB SHAs due to rebases)
    let existing_shas: std::collections::HashSet<String> = references
        .iter()
        .filter(|r| r.id.is_some())
        .map(|r| r.sha.clone())
        .collect();

    let db_commit_sessions: Vec<(Uuid, String, String, Uuid, String, Option<String>)> =
        sqlx::query_as(
            "SELECT DISTINCT c.id, c.commit_sha, \
                    COALESCE(c.message, ''), \
                    s.id, s.session_id, s.model \
             FROM commits c \
             JOIN commit_attributions ca ON ca.commit_id = c.id \
             JOIN sessions s ON s.id = ca.session_id \
             WHERE c.repo_id = $1 AND ca.file_path = $2",
        )
        .bind(repo_id)
        .bind(&req.path)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();

    // Group by commit SHA and build supplementary refs
    {
        use std::collections::HashMap;
        let mut supp_map: HashMap<String, CommitRef> = HashMap::new();
        for (c_id, sha, msg, s_id, s_session_id, s_model) in &db_commit_sessions {
            if existing_shas.contains(sha) {
                // Update existing ref that had id=null with the DB id
                if let Some(existing) = references
                    .iter_mut()
                    .find(|r| r.sha == *sha && r.id.is_none())
                {
                    existing.id = Some(c_id.to_string());
                    existing.sessions.push(SessionRefResponse {
                        id: s_id.to_string(),
                        session_id: s_session_id.clone(),
                        model: s_model.clone(),
                    });
                }
                continue;
            }
            supp_map
                .entry(sha.clone())
                .and_modify(|r| {
                    let sid = s_id.to_string();
                    if !r.sessions.iter().any(|s| s.id == sid) {
                        r.sessions.push(SessionRefResponse {
                            id: sid,
                            session_id: s_session_id.clone(),
                            model: s_model.clone(),
                        });
                    }
                })
                .or_insert_with(|| CommitRef {
                    sha: sha.clone(),
                    id: Some(c_id.to_string()),
                    message: msg.clone(),
                    author: String::new(),
                    sessions: vec![SessionRefResponse {
                        id: s_id.to_string(),
                        session_id: s_session_id.clone(),
                        model: s_model.clone(),
                    }],
                });
        }
        references.extend(supp_map.into_values());
    }

    let commits_analyzed: Vec<String> = ctx.changes.iter().map(|c| c.commit_sha.clone()).collect();
    let sessions_referenced: Vec<String> = references
        .iter()
        .flat_map(|r| r.sessions.iter().map(|s| s.session_id.clone()))
        .collect();

    save_story_cache(
        &state.pool,
        repo_id,
        &req.path,
        &scope.name,
        scope.start_line,
        scope.end_line,
        &req.git_ref,
        &head_sha,
        &story_md,
        &commits_analyzed,
        &sessions_referenced,
        &references,
        story_provider.provider_name(),
        story_provider.model_name(),
    )
    .await;

    Ok(Json(StoryResponse {
        story: story_md,
        function_name: scope.name,
        kind: scope.kind,
        line_range: [scope.start_line, scope.end_line],
        commits_analyzed,
        sessions_referenced,
        references,
        cached: false,
        generated_at: chrono::Utc::now().to_rfc3339(),
    }))
}

// --- Function sessions endpoint ---

pub async fn get_function_sessions(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, repo_id)): Path<(String, Uuid)>,
    Query(query): Query<FunctionSessionsQuery>,
) -> Result<Json<FunctionSessionsResponse>, AppError> {
    if !state
        .extensions
        .permissions
        .has_permission(&auth.role, Permission::CodeBrowse)
    {
        return Err(AppError::Forbidden("Insufficient permissions".into()));
    }

    verify_repo_access(&state, auth.org_id, repo_id).await?;

    // Detect function scope at the clicked line (git2 types are !Send)
    let scope = {
        let repo = state
            .repo_manager
            .open_repo(repo_id)
            .map_err(AppError::internal)?;

        let ext = query.path.rsplit('.').next().unwrap_or("");

        let obj = repo
            .revparse_single(&query.git_ref)
            .map_err(|e| AppError::BadRequest(format!("Invalid ref: {e}")))?;
        let commit = obj
            .peel_to_commit()
            .map_err(|e| AppError::BadRequest(format!("Ref is not a commit: {e}")))?;
        let tree = commit.tree()?;
        let entry = tree
            .get_path(std::path::Path::new(&query.path))
            .map_err(|e| AppError::NotFound(format!("File not found: {e}")))?;
        let blob_obj = entry.to_object(&repo)?;
        let blob = blob_obj
            .as_blob()
            .ok_or_else(|| AppError::BadRequest("Path is not a file".into()))?;
        let source = String::from_utf8_lossy(blob.content()).to_string();

        tracevault_core::code_nav::find_enclosing_scope(&source, ext, query.line)
            .unwrap_or_else(|| tracevault_core::code_nav::fallback_scope(&source, query.line, 20))
    };

    let result = crate::story::gather_function_sessions(
        &state.repo_manager,
        &state.pool,
        repo_id,
        &query.git_ref,
        &query.path,
        &scope,
    )
    .await
    .map_err(AppError::internal)?;

    Ok(Json(FunctionSessionsResponse {
        function_name: result.function_name,
        line_range: [result.line_range.0, result.line_range.1],
        sessions: result
            .sessions
            .into_iter()
            .map(|s| FunctionSessionRefResponse {
                id: s.id.to_string(),
                session_id: s.session_id,
                model: s.model,
                user_email: s.user_email,
                started_at: s.started_at.map(|t| t.to_rfc3339()),
                commit_shas: s.commit_shas,
            })
            .collect(),
    }))
}

// --- Story cache helpers ---

async fn check_story_cache(
    pool: &sqlx::PgPool,
    repo_id: Uuid,
    file_path: &str,
    function_name: &str,
    ref_name: &str,
    head_sha: &str,
) -> Option<StoryResponse> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            i32,
            i32,
            serde_json::Value,
            serde_json::Value,
            serde_json::Value,
            String,
        ),
    >(
        "SELECT story_markdown, function_name, line_range_start, line_range_end, \
         commits_analyzed, sessions_referenced, references_data, generated_at::text \
         FROM code_stories WHERE repo_id = $1 AND file_path = $2 AND function_name = $3 \
         AND ref_name = $4 AND head_commit_sha = $5 LIMIT 1",
    )
    .bind(repo_id)
    .bind(file_path)
    .bind(function_name)
    .bind(ref_name)
    .bind(head_sha)
    .fetch_optional(pool)
    .await
    .ok()??;

    Some(StoryResponse {
        story: row.0,
        function_name: row.1,
        kind: "cached".to_string(),
        line_range: [row.2 as usize, row.3 as usize],
        commits_analyzed: serde_json::from_value(row.4).unwrap_or_default(),
        sessions_referenced: serde_json::from_value(row.5).unwrap_or_default(),
        references: serde_json::from_value(row.6).unwrap_or_default(),
        cached: true,
        generated_at: row.7,
    })
}

#[allow(clippy::too_many_arguments)]
async fn save_story_cache(
    pool: &sqlx::PgPool,
    repo_id: Uuid,
    file_path: &str,
    function_name: &str,
    start: usize,
    end: usize,
    ref_name: &str,
    head_sha: &str,
    story: &str,
    commits: &[String],
    sessions: &[String],
    references: &[CommitRef],
    provider: &str,
    model: &str,
) {
    sqlx::query("DELETE FROM code_stories WHERE repo_id = $1 AND file_path = $2 AND function_name = $3 AND ref_name = $4")
        .bind(repo_id)
        .bind(file_path)
        .bind(function_name)
        .bind(ref_name)
        .execute(pool)
        .await
        .ok();

    sqlx::query(
        "INSERT INTO code_stories (repo_id, file_path, function_name, line_range_start, \
         line_range_end, ref_name, head_commit_sha, story_markdown, commits_analyzed, \
         sessions_referenced, references_data, llm_provider, llm_model) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
    )
    .bind(repo_id)
    .bind(file_path)
    .bind(function_name)
    .bind(start as i32)
    .bind(end as i32)
    .bind(ref_name)
    .bind(head_sha)
    .bind(story)
    .bind(serde_json::json!(commits))
    .bind(serde_json::json!(sessions))
    .bind(serde_json::json!(references))
    .bind(provider)
    .bind(model)
    .execute(pool)
    .await
    .ok();
}

// --- Per-org LLM resolution ---

pub async fn resolve_org_llm(state: &AppState, org_id: Uuid) -> Option<Box<dyn crate::llm::StoryLlm>> {
    let row = sqlx::query_as::<
        _,
        (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(
        "SELECT llm_provider, llm_api_key_encrypted, llm_api_key_nonce, llm_model, llm_base_url
         FROM org_compliance_settings WHERE org_id = $1",
    )
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await
    .ok()??;

    let provider = row.0?;
    let encrypted_key = row.1?;
    let nonce = row.2?;

    let api_key = state
        .extensions
        .encryption
        .decrypt(&encrypted_key, &nonce)
        .map_err(|e| tracing::error!("Failed to decrypt org LLM API key: {e}"))
        .ok()?;

    crate::llm::create_llm_from_params(&provider, api_key, row.3, row.4)
}
