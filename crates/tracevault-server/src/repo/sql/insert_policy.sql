INSERT INTO policies (org_id, repo_id, name, description, condition, action, severity, scope, enabled)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
RETURNING id, created_at, updated_at
