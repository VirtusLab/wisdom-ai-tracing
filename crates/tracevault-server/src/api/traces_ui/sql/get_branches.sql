-- Per-branch cost, summed over each branch's DISTINCT attributed sessions.
--
-- A session attributed to several commits on a branch must be counted once,
-- so we cannot SUM in the outer query (the branch_tracking × commits ×
-- commit_attributions fan-out repeats a session per commit). SUM(DISTINCT ...)
-- is also wrong — it dedups by cost *value*, collapsing sessions that share a
-- cost (notably $0.00 when pricing isn't configured).
--
-- Instead, pre-aggregate cost per (branch, repo_id) in a CTE that is scoped to
-- the same org/repo as the outer query, then LEFT JOIN it — one cost row per
-- branch, no correlated subquery per output row.
WITH branch_session_costs AS (
    SELECT uniq.branch, uniq.repo_id, SUM(s.estimated_cost_usd) AS total_cost
    FROM (
        SELECT DISTINCT bt.branch, c.repo_id, ca.session_id
        FROM branch_tracking bt
        JOIN commits c ON c.id = bt.commit_id
        JOIN repos r ON r.id = c.repo_id
        JOIN commit_attributions ca ON ca.commit_id = c.id
        WHERE r.org_id = $1
          AND ($2::uuid IS NULL OR c.repo_id = $2)
          AND ca.session_id IS NOT NULL
    ) AS uniq
    JOIN sessions s ON s.id = uniq.session_id
    GROUP BY uniq.branch, uniq.repo_id
)
SELECT
    bt.branch,
    r.name AS repo_name,
    MAX(bt.tag) AS tag,
    COUNT(DISTINCT bt.commit_id) AS commits_count,
    COUNT(DISTINCT ca.session_id) AS sessions_count,
    COALESCE(MAX(bsc.total_cost), 0) AS total_cost,
    MAX(bt.tracking_type) AS status,
    MAX(bt.tracked_at) AS last_activity
FROM branch_tracking bt
JOIN commits c ON bt.commit_id = c.id
JOIN repos r ON c.repo_id = r.id
LEFT JOIN commit_attributions ca ON ca.commit_id = c.id
LEFT JOIN branch_session_costs bsc ON bsc.branch = bt.branch AND bsc.repo_id = c.repo_id
WHERE r.org_id = $1
  AND ($2::uuid IS NULL OR c.repo_id = $2)
GROUP BY bt.branch, r.name, c.repo_id
ORDER BY last_activity DESC NULLS LAST
