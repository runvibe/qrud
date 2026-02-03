CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    deleted_at BIGINT
);

CREATE TABLE IF NOT EXISTS documents (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id),
    pk TEXT NOT NULL,
    data TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    deleted_at BIGINT
);

CREATE UNIQUE INDEX IF NOT EXISTS documents_workspace_pk_idx
    ON documents(workspace_id, pk);

CREATE INDEX IF NOT EXISTS documents_workspace_idx
    ON documents(workspace_id);

CREATE INDEX IF NOT EXISTS documents_workspace_name_idx
    ON documents(workspace_id, (data::jsonb->>'name'));

CREATE INDEX IF NOT EXISTS documents_workspace_title_idx
    ON documents(workspace_id, (data::jsonb->>'title'));

CREATE INDEX IF NOT EXISTS documents_workspace_label_idx
    ON documents(workspace_id, (data::jsonb->>'label'));

CREATE INDEX IF NOT EXISTS documents_workspace_description_idx
    ON documents(workspace_id, (data::jsonb->>'description'));
