SELECT COUNT(*),
       COALESCE(CAST(SUM(s.total_tokens) AS BIGINT), 0),
       COALESCE(SUM(s.estimated_cost_usd), 0.0),
       CAST(AVG(COALESCE(NULLIF(s.duration_ms, 0), CASE WHEN s.ended_at IS NOT NULL AND s.started_at IS NOT NULL THEN EXTRACT(EPOCH FROM (s.ended_at - s.started_at))::BIGINT * 1000 ELSE NULL END)) AS BIGINT),
       COALESCE(CAST(SUM(s.total_tool_calls) AS BIGINT), 0),
       COALESCE(CAST(SUM(s.input_tokens) AS BIGINT), 0),
       COALESCE(CAST(SUM(s.output_tokens) AS BIGINT), 0),
       COALESCE(CAST(SUM(s.cache_read_tokens) AS BIGINT), 0),
       COALESCE(CAST(SUM(s.cache_write_tokens) AS BIGINT), 0)
FROM sessions s
JOIN repos r ON s.repo_id = r.id
WHERE r.org_id = $1 AND s.user_id = $2
  AND ($3::TEXT IS NULL OR r.name = $3)
  AND ($4::TIMESTAMPTZ IS NULL OR s.created_at >= $4)
  AND ($5::TIMESTAMPTZ IS NULL OR s.created_at <= $5)
