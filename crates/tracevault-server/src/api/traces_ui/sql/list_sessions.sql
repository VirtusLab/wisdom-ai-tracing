SELECT s.id, s.session_id, s.repo_id, r.name AS repo_name,
       s.user_id, u.email AS user_email, s.status, s.model, s.tool,
       s.total_tool_calls, s.total_tokens, s.estimated_cost_usd,
       s.cwd, s.started_at, s.updated_at,
       COUNT(*) OVER() AS total_count
FROM sessions s
JOIN repos r ON s.repo_id = r.id
JOIN users u ON s.user_id = u.id
WHERE r.org_id = $1
  AND ($2::UUID IS NULL OR s.repo_id = $2)
  AND ($3::TEXT IS NULL OR s.status = $3)
  AND ($4::BOOL = FALSE OR s.updated_at < now() - interval '30 minutes')
  AND ($5::TIMESTAMPTZ IS NULL OR s.started_at >= $5)
  AND ($6::TIMESTAMPTZ IS NULL OR s.started_at <= $6)
ORDER BY s.updated_at DESC
LIMIT $7 OFFSET $8
