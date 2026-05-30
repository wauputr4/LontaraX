PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS providers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    base_url TEXT NOT NULL,
    api_key TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS models (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    model_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    capability TEXT NOT NULL CHECK (capability IN ('chat', 'image', 'video', 'audio', 'embedding')),
    metadata_json TEXT NOT NULL DEFAULT '{}',
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    UNIQUE(provider, model_id, capability)
);

CREATE TABLE IF NOT EXISTS requests (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    capability TEXT NOT NULL,
    status TEXT NOT NULL,
    cost REAL,
    input_hash TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    external_task_id TEXT NOT NULL,
    status TEXT NOT NULL,
    result_url TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_models_provider_capability ON models(provider, capability, enabled);
CREATE INDEX IF NOT EXISTS idx_requests_provider_created_at ON requests(provider, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_tasks_provider_created_at ON tasks(provider, created_at DESC);
