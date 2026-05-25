SELECT ca.commit_id, ca.session_id, ca.line_start, ca.line_end, ca.confidence
FROM commit_attributions ca
JOIN sessions s ON ca.session_id = s.id
WHERE ca.commit_id = ANY($1) AND ca.file_path = $2
