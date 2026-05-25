SELECT ca.file_path, ca.session_id, s.session_id AS session_short_id,
       MAX(ca.confidence) AS confidence,
       MIN(ca.line_start) AS line_start,
       MAX(ca.line_end) AS line_end
FROM commit_attributions ca
JOIN sessions s ON ca.session_id = s.id
WHERE ca.commit_id = $1
GROUP BY ca.file_path, ca.session_id, s.session_id
ORDER BY ca.file_path, MIN(ca.line_start) NULLS LAST
