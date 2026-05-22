ALTER TABLE policy_evaluations
    ADD COLUMN is_synthetic BOOLEAN NOT NULL DEFAULT false;
