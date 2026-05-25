SELECT COUNT(*)
FROM sessions s
JOIN repos r ON s.repo_id = r.id
WHERE r.org_id = $1
  AND ($2::UUID IS NULL OR s.repo_id = $2)
  AND ($3::TEXT IS NULL OR s.status = $3)
  AND ($4::BOOL = FALSE OR s.updated_at < now() - interval '30 minutes')
  AND ($5::TIMESTAMPTZ IS NULL OR s.started_at >= $5)
  AND ($6::TIMESTAMPTZ IS NULL OR s.started_at <= $6)
