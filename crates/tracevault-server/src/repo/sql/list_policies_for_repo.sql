SELECT id, org_id, repo_id, name, description, condition, action, severity, scope, enabled, created_at, updated_at
FROM policies
WHERE org_id = $1 AND (repo_id = $2 OR repo_id IS NULL)
ORDER BY created_at
