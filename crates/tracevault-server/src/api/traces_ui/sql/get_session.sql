SELECT s.id, s.session_id, r.name AS repo_name, u.email AS user_email,
       s.status, s.model, s.tool, s.total_tool_calls, s.total_tokens,
       s.input_tokens, s.output_tokens, s.cache_read_tokens, s.cache_write_tokens,
       s.estimated_cost_usd, s.cwd, s.started_at, s.ended_at, s.updated_at
FROM sessions s
JOIN repos r ON s.repo_id = r.id
JOIN users u ON s.user_id = u.id
WHERE s.id = $1 AND r.org_id = $2
