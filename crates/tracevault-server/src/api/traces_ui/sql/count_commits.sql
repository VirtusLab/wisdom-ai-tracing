SELECT COUNT(DISTINCT c.id)
FROM commits c
JOIN repos r ON c.repo_id = r.id
WHERE r.org_id = $1
  AND ($2::UUID IS NULL OR c.repo_id = $2)
  AND ($3::TEXT IS NULL OR c.branch = $3)
  AND ($4::TIMESTAMPTZ IS NULL OR c.committed_at >= $4)
  AND ($5::TIMESTAMPTZ IS NULL OR c.committed_at <= $5)
