SELECT github_url, clone_status, deploy_key_encrypted, deploy_key_nonce, webhook_secret_encrypted, last_fetched_at, verification_phase_mode, clone_started_at, clone_error
FROM repos
WHERE id = $1
