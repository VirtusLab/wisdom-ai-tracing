SELECT COUNT(DISTINCT ca.commit_id)
FROM commit_attributions ca
JOIN commits c ON ca.commit_id = c.id
WHERE ca.session_id = $1
