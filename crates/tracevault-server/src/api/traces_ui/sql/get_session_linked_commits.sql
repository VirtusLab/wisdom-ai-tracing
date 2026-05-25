SELECT ca.commit_id, c.commit_sha, c.branch, MAX(ca.confidence) AS confidence
FROM commit_attributions ca
JOIN commits c ON ca.commit_id = c.id
WHERE ca.session_id = $1
GROUP BY ca.commit_id, c.commit_sha, c.branch, c.committed_at
ORDER BY c.committed_at DESC NULLS LAST
