SELECT github_url, clone_status, deploy_key_encrypted, webhook_secret_encrypted, last_fetched_at, validation_window_mode
FROM repos
WHERE id = $1 AND org_id = $2
