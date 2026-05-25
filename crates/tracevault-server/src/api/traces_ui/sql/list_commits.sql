SELECT c.id, c.commit_sha, c.branch, c.author, c.message,
       COUNT(DISTINCT ca.file_path) AS files_changed,
       COUNT(DISTINCT ca.session_id) AS ai_sessions_count,
       c.committed_at,
       COUNT(*) OVER() AS total_count
FROM commits c
JOIN repos r ON c.repo_id = r.id
LEFT JOIN commit_attributions ca ON ca.commit_id = c.id
WHERE r.org_id = $1
  AND ($2::UUID IS NULL OR c.repo_id = $2)
  AND ($3::TEXT IS NULL OR c.branch = $3)
  AND ($4::TIMESTAMPTZ IS NULL OR c.committed_at >= $4)
  AND ($5::TIMESTAMPTZ IS NULL OR c.committed_at <= $5)
GROUP BY c.id
ORDER BY c.committed_at DESC NULLS LAST
LIMIT $6 OFFSET $7
