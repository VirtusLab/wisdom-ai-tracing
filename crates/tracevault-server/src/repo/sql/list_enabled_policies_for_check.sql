SELECT id, name, condition, action, severity, scope
FROM policies
WHERE org_id = $1 AND (repo_id = $2 OR repo_id IS NULL) AND enabled = true
ORDER BY created_at
