SELECT c.id, c.commit_sha, c.branch, c.author, c.message, c.committed_at
FROM commits c
JOIN repos r ON c.repo_id = r.id
WHERE c.id = $1 AND r.org_id = $2
