SELECT
    bt.branch,
    MAX(bt.tag) AS tag,
    COUNT(DISTINCT bt.commit_id) AS commits_count,
    COUNT(DISTINCT ca.session_id) AS sessions_count,
    COALESCE(SUM(DISTINCT s.estimated_cost_usd), 0) AS total_cost,
    MAX(bt.tracking_type) AS status,
    MAX(bt.tracked_at) AS last_activity
FROM branch_tracking bt
JOIN commits c ON bt.commit_id = c.id
JOIN repos r ON c.repo_id = r.id
LEFT JOIN commit_attributions ca ON ca.commit_id = c.id
LEFT JOIN sessions s ON ca.session_id = s.id
WHERE r.org_id = $1
  AND ($2::uuid IS NULL OR c.repo_id = $2)
GROUP BY bt.branch
ORDER BY last_activity DESC NULLS LAST
