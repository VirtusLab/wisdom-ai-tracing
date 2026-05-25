SELECT commit_sha, id FROM commits WHERE repo_id = $1 AND commit_sha = ANY($2)
