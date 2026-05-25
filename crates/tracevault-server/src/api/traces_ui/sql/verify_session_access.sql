SELECT EXISTS(
    SELECT 1 FROM sessions s
    JOIN repos r ON s.repo_id = r.id
    WHERE s.id = $1 AND r.org_id = $2
)
