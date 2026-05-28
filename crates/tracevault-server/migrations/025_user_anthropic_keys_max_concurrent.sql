-- Per-credential concurrency cap for the transparent Anthropic LLM proxy
-- (issue softwaremill/tracevault#210, parent #181).
--
-- The cap is the maximum number of in-flight proxy requests this credential
-- can have at any one moment. Enforced in-process via a tokio Semaphore in
-- AppState, sized to this value at first use of the credential.
--
-- Default 8: comfortable for typical multi-agent setups (Claude Code + GSD2),
-- well under any paid Anthropic tier. Upper bound 256 prevents user-typed
-- nonsense values; lower bound 1 prevents accidental lockout.
ALTER TABLE user_anthropic_keys
    ADD COLUMN max_concurrent INTEGER NOT NULL DEFAULT 8
    CHECK (max_concurrent > 0 AND max_concurrent <= 256);
