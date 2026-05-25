SELECT id, LEFT(session_id, 8) FROM sessions WHERE id = ANY($1)
