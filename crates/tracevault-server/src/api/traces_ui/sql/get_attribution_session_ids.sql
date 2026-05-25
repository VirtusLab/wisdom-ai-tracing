SELECT s.id, LEFT(s.session_id, 8)
FROM sessions s
JOIN repos r ON r.id = s.repo_id
WHERE s.id = ANY($1)
  AND r.org_id = $2
