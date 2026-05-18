SELECT COUNT(*)
FROM policy_evaluations
WHERE org_id = $1 AND repo_id = $2
  AND ($3::uuid IS NULL OR policy_id = $3)
  AND ($4::text IS NULL OR result = $4)
  AND ($5::text IS NULL OR source = $5)
  AND ($6::timestamptz IS NULL OR evaluated_at >= $6)
