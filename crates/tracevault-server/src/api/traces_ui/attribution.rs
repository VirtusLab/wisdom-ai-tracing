use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::{extractors::OrgAuth, AppState};

#[derive(Debug, Serialize)]
pub struct AttributionResponse {
    pub file_path: String,
    pub commit_sha: String,
    pub lines: Vec<AttributionLine>,
}

#[derive(Debug, Serialize)]
pub struct AttributionLine {
    pub line_number: usize,
    pub content: String,
    pub git_author: Option<String>,
    pub session_id: Option<String>,
    pub session_short_id: Option<String>,
    pub confidence: Option<f32>,
}

/// GET /api/v1/orgs/{slug}/traces/attribution/{commit_id}/{*file_path}
pub async fn get_attribution(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, commit_id, file_path)): Path<(String, Uuid, String)>,
) -> Result<Json<AttributionResponse>, AppError> {
    let row = sqlx::query_as::<_, (String, Uuid)>(
        "SELECT c.commit_sha, c.repo_id
         FROM commits c
         JOIN repos r ON c.repo_id = r.id
         WHERE c.id = $1 AND r.org_id = $2",
    )
    .bind(commit_id)
    .bind(auth.org_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Commit not found".into()))?;

    let (commit_sha, repo_id) = row;

    let clone_path =
        sqlx::query_scalar::<_, Option<String>>("SELECT clone_path FROM repos WHERE id = $1")
            .bind(repo_id)
            .fetch_one(&state.pool)
            .await?
            .ok_or_else(|| AppError::BadRequest("Repo not cloned".into()))?;

    let file_content = std::process::Command::new("git")
        .args(["show", &format!("{commit_sha}:{file_path}")])
        .current_dir(&clone_path)
        .output()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if !file_content.status.success() {
        return Err(AppError::NotFound("File not found at this commit".into()));
    }

    let content = String::from_utf8_lossy(&file_content.stdout);
    let content_lines: Vec<&str> = content.lines().collect();

    let blame_output = std::process::Command::new("git")
        .args(["blame", "--porcelain", &commit_sha, "--", &file_path])
        .current_dir(&clone_path)
        .output()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let blame_text = String::from_utf8_lossy(&blame_output.stdout);
    let blame_map = parse_porcelain_blame(&blame_text);

    let blame_shas: Vec<String> = blame_map
        .values()
        .map(|b| b.commit_sha.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let sha_to_commit_id: std::collections::HashMap<String, Uuid> = if !blame_shas.is_empty() {
        sqlx::query_as::<_, (String, Uuid)>(
            "SELECT commit_sha, id FROM commits WHERE repo_id = $1 AND commit_sha = ANY($2)",
        )
        .bind(repo_id)
        .bind(&blame_shas)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .collect()
    } else {
        std::collections::HashMap::new()
    };

    let all_commit_ids: Vec<Uuid> = sha_to_commit_id.values().copied().collect();

    let attributions = sqlx::query_as::<_, (Uuid, Option<Uuid>, i32, i32, f32)>(
        "SELECT ca.commit_id, ca.session_id, ca.line_start, ca.line_end, ca.confidence
         FROM commit_attributions ca
         JOIN sessions s ON ca.session_id = s.id
         WHERE ca.commit_id = ANY($1) AND ca.file_path = $2",
    )
    .bind(&all_commit_ids)
    .bind(&file_path)
    .fetch_all(&state.pool)
    .await?;

    let session_ids: Vec<Uuid> = attributions.iter().filter_map(|a| a.1).collect();
    let session_short_ids: std::collections::HashMap<Uuid, String> = if !session_ids.is_empty() {
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, LEFT(session_id, 8) FROM sessions WHERE id = ANY($1)",
        )
        .bind(&session_ids)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .collect()
    } else {
        std::collections::HashMap::new()
    };

    let mut lines = Vec::new();
    for (i, line_content) in content_lines.iter().enumerate() {
        let line_num = i + 1;
        let blame_info = blame_map.get(&line_num);
        let git_author = blame_info.map(|b| b.author.clone());

        let line_commit_id = blame_info
            .and_then(|b| sha_to_commit_id.get(&b.commit_sha))
            .copied();

        let mut best_session: Option<Uuid> = None;
        let mut best_confidence: Option<f32> = None;

        for (cid, sid, start, end, conf) in &attributions {
            if line_num as i32 >= *start && line_num as i32 <= *end {
                let is_blame_commit = line_commit_id == Some(*cid);
                let is_better = match best_confidence {
                    None => true,
                    Some(bc) => is_blame_commit || *conf > bc,
                };
                if is_better {
                    best_session = *sid;
                    best_confidence = Some(*conf);
                }
            }
        }

        if best_session.is_none() {
            if let Some(blame_cid) = line_commit_id {
                for (cid, sid, _start, _end, conf) in &attributions {
                    if *cid == blame_cid {
                        let is_better = match best_confidence {
                            None => true,
                            Some(bc) => *conf > bc,
                        };
                        if is_better {
                            best_session = *sid;
                            best_confidence = Some(*conf);
                        }
                    }
                }
            }
        }

        lines.push(AttributionLine {
            line_number: line_num,
            content: line_content.to_string(),
            git_author,
            session_id: best_session.map(|s| s.to_string()),
            session_short_id: best_session.and_then(|s| session_short_ids.get(&s).cloned()),
            confidence: best_confidence,
        });
    }

    Ok(Json(AttributionResponse {
        file_path,
        commit_sha,
        lines,
    }))
}

struct BlameInfo {
    author: String,
    commit_sha: String,
}

fn parse_porcelain_blame(text: &str) -> std::collections::HashMap<usize, BlameInfo> {
    let mut map = std::collections::HashMap::new();
    let mut current_line: usize = 0;
    let mut current_author = String::new();
    let mut current_sha = String::new();

    for line in text.lines() {
        if line.len() >= 40 && line.chars().take(40).all(|c| c.is_ascii_hexdigit()) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                current_sha = parts[0].to_string();
                current_line = parts[2].parse().unwrap_or(0);
            }
        } else if let Some(author) = line.strip_prefix("author ") {
            current_author = author.to_string();
        } else if line.starts_with('\t') && current_line > 0 {
            map.insert(
                current_line,
                BlameInfo {
                    author: current_author.clone(),
                    commit_sha: current_sha.clone(),
                },
            );
        }
    }

    map
}
