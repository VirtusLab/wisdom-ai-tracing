SELECT COUNT(*) FROM events e
JOIN sessions s ON e.session_id = s.id
JOIN repos r ON s.repo_id = r.id
WHERE r.org_id = $1
  AND ($2::UUID IS NULL OR s.repo_id = $2)
