SELECT chunk_index, data
FROM transcript_chunks
WHERE session_id = $1
ORDER BY chunk_index ASC
