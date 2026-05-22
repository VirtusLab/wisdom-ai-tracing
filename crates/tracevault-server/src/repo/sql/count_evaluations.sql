SELECT COUNT(*)
FROM policy_evaluations
WHERE org_id = $1 AND repo_id = $2
  AND ($3::uuid IS NULL OR policy_id = $3)
  AND ($4::text IS NULL OR result = $4)
  AND ($5::text IS NULL OR action = $5)
  AND ($6::text IS NULL OR source = $6)
  AND ($7::timestamptz IS NULL OR evaluated_at >= $7)
  AND ($8::timestamptz IS NULL OR evaluated_at <= $8)
