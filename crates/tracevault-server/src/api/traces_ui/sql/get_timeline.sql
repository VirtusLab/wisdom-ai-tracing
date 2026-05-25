SELECT item_type, event_id, session_id, session_short_id, event_type, tool_name, file_path, commit_sha, branch, author, timestamp FROM (
    SELECT 'event'::text AS item_type,
           e.id AS event_id,
           e.session_id,
           LEFT(s.session_id, 8) AS session_short_id,
           e.event_type,
           e.tool_name,
           e.tool_input->>'file_path' AS file_path,
           NULL::text AS commit_sha,
           NULL::text AS branch,
           NULL::text AS author,
           e.timestamp
    FROM events e
    JOIN sessions s ON e.session_id = s.id
    JOIN repos r ON s.repo_id = r.id
    WHERE r.org_id = $1
      AND ($2::uuid IS NULL OR s.repo_id = $2)
      AND ($3::text IS NULL OR e.tool_name = $3)
      AND ($4::uuid IS NULL OR e.session_id = $4)
      AND ($5::timestamptz IS NULL OR e.timestamp >= $5)
      AND ($6::timestamptz IS NULL OR e.timestamp <= $6)

    UNION ALL

    SELECT 'commit'::text AS item_type,
           NULL::uuid AS event_id,
           NULL::uuid AS session_id,
           NULL::text AS session_short_id,
           NULL::text AS event_type,
           NULL::text AS tool_name,
           NULL::text AS file_path,
           c.commit_sha,
           c.branch,
           c.author,
           c.committed_at AS timestamp
    FROM commits c
    JOIN repos r ON c.repo_id = r.id
    WHERE r.org_id = $1
      AND ($2::uuid IS NULL OR c.repo_id = $2)
      AND ($5::timestamptz IS NULL OR c.committed_at >= $5)
      AND ($6::timestamptz IS NULL OR c.committed_at <= $6)
      AND $3::text IS NULL
      AND $4::uuid IS NULL
) combined
ORDER BY timestamp DESC
LIMIT $7 OFFSET $8
