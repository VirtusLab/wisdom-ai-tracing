-- Pre-aggregate unique (branch, repo_id, session_id) tuples and join costs,
-- so the outer query can SUM session costs without a correlated subquery per row.
WITH branch_unique_sessions AS (
    SELECT DISTINCT
        bt.branch,
        c.repo_id,
        ca.session_id
    FROM branch_tracking bt
    JOIN commits c ON c.id = bt.commit_id
    LEFT JOIN commit_attributions ca ON ca.commit_id = c.id
    WHERE ca.session_id IS NOT NULL
)
SELECT
    bt.branch,
    r.name AS repo_name,
    MAX(bt.tag) AS tag,
    COUNT(DISTINCT bt.commit_id) AS commits_count,
    COUNT(DISTINCT ca.session_id) AS sessions_count,
    COALESCE((
        SELECT SUM(s2.estimated_cost_usd)
        FROM branch_unique_sessions bus
        JOIN sessions s2 ON s2.id = bus.session_id
        WHERE bus.branch = bt.branch AND bus.repo_id = c.repo_id
    ), 0) AS total_cost,
    MAX(bt.tracking_type) AS status,
    MAX(bt.tracked_at) AS last_activity
FROM branch_tracking bt
JOIN commits c ON bt.commit_id = c.id
JOIN repos r ON c.repo_id = r.id
LEFT JOIN commit_attributions ca ON ca.commit_id = c.id
LEFT JOIN sessions s ON ca.session_id = s.id
WHERE r.org_id = $1
  AND ($2::uuid IS NULL OR c.repo_id = $2)
GROUP BY bt.branch, r.name, c.repo_id
ORDER BY last_activity DESC NULLS LAST
