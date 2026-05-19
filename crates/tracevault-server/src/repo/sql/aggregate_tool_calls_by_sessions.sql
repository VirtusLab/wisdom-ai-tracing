SELECT e.tool_name,
       COUNT(*) AS total,
       COUNT(*) FILTER (WHERE e.is_error = false) AS successful
FROM events e
WHERE e.session_id = ANY($1) AND e.tool_name IS NOT NULL
GROUP BY e.tool_name
