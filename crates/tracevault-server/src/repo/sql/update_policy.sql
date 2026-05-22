UPDATE policies SET
    name = COALESCE($3, name),
    description = COALESCE($4, description),
    condition = COALESCE($5, condition),
    action = COALESCE($6, action),
    severity = COALESCE($7, severity),
    scope = COALESCE($8, scope),
    enabled = COALESCE($9, enabled),
    updated_at = NOW()
WHERE id = $1 AND org_id = $2
RETURNING org_id, repo_id, name, description, condition, action, severity, scope, enabled, created_at, updated_at
