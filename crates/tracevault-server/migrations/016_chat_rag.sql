-- 016_chat_rag.sql
-- Enterprise Chat RAG: embedding tables, chat persistence
-- pgvector-dependent objects are created conditionally (enterprise-only)

-- pgvector extension + embedding tables: only if the extension is available.
-- Community installs without pgvector skip this block safely.
DO $$
BEGIN
    -- Try to enable pgvector; skip everything vector-related if unavailable
    BEGIN
        CREATE EXTENSION IF NOT EXISTS vector;
    EXCEPTION WHEN OTHERS THEN
        RAISE NOTICE 'pgvector extension not available — skipping embedding tables (enterprise-only feature)';
        RETURN;
    END;

    -- Session embeddings (Tier 1: one summary vector per session)
    CREATE TABLE IF NOT EXISTS session_embeddings (
        id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
        session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        summary TEXT NOT NULL,
        embedding VECTOR(384) NOT NULL,
        embedding_model_version TEXT NOT NULL,
        created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
        UNIQUE(session_id)
    );

    CREATE INDEX IF NOT EXISTS idx_session_embeddings_vector
        ON session_embeddings USING hnsw (embedding vector_cosine_ops);

    -- Chunk embeddings (Tier 2: sliding window vectors per session)
    CREATE TABLE IF NOT EXISTS chunk_embeddings (
        id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
        session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        chunk_start INTEGER NOT NULL,
        chunk_end INTEGER NOT NULL,
        content_preview TEXT NOT NULL,
        embedding VECTOR(384) NOT NULL,
        embedding_model_version TEXT NOT NULL,
        created_at TIMESTAMPTZ NOT NULL DEFAULT now()
    );

    CREATE INDEX IF NOT EXISTS idx_chunk_embeddings_vector
        ON chunk_embeddings USING hnsw (embedding vector_cosine_ops);
    CREATE INDEX IF NOT EXISTS idx_chunk_embeddings_session
        ON chunk_embeddings (session_id);
END
$$;

-- Non-vector tables: always created (lightweight, no extension needed)

-- Indexing progress tracking
CREATE TABLE IF NOT EXISTS chat_indexing_status (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'pending',
    error_text TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(session_id)
);

-- Chat conversations
CREATE TABLE IF NOT EXISTS chat_conversations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_chat_conversations_user
    ON chat_conversations (user_id, updated_at DESC);

-- Chat messages
CREATE TABLE IF NOT EXISTS chat_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES chat_conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    referenced_sessions UUID[],
    referenced_commits TEXT[],
    filters_applied JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_conversation
    ON chat_messages (conversation_id, created_at);

-- Summarization LLM config (separate from existing stories LLM)
DO $$
BEGIN
    ALTER TABLE org_compliance_settings
        ADD COLUMN chat_summarization_provider TEXT,
        ADD COLUMN chat_summarization_model TEXT,
        ADD COLUMN chat_summarization_api_key_encrypted TEXT,
        ADD COLUMN chat_summarization_api_key_nonce TEXT,
        ADD COLUMN chat_summarization_base_url TEXT,
        ADD COLUMN chat_auto_summarize BOOLEAN NOT NULL DEFAULT false;
EXCEPTION WHEN duplicate_column THEN
    -- Columns already exist, nothing to do
    NULL;
END
$$;
