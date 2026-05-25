SELECT COUNT(*) FROM sessions s
JOIN repos r ON s.repo_id = r.id
WHERE r.org_id = $1
  AND s.status = 'active'
  AND s.updated_at >= now() - interval '30 minutes'
  AND ($2::UUID IS NULL OR s.repo_id = $2)
