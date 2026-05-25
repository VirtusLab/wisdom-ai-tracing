SELECT COUNT(*) FROM commits c
JOIN repos r ON c.repo_id = r.id
WHERE r.org_id = $1
  AND ($2::UUID IS NULL OR c.repo_id = $2)
