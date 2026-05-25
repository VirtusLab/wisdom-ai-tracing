SELECT c.commit_sha, c.repo_id
FROM commits c
JOIN repos r ON c.repo_id = r.id
WHERE c.id = $1 AND r.org_id = $2
