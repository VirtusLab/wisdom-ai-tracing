SELECT github_url, clone_status, deploy_key_encrypted, webhook_secret_encrypted, last_fetched_at, verification_phase_mode, clone_error
FROM repos
WHERE id = $1 AND org_id = $2
