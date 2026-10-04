ALTER TABLE users
    ADD COLUMN IF NOT EXISTS username text
    CHECK (username IS NULL OR username ~ '^[a-z0-9][a-z0-9_-]{2,31}$');

CREATE UNIQUE INDEX IF NOT EXISTS users_username_unique_idx
    ON users (username) WHERE username IS NOT NULL;

CREATE TABLE member_invitations (
    tenant_id uuid NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    id uuid NOT NULL,
    username text NOT NULL CHECK (username ~ '^[a-z0-9][a-z0-9_-]{2,31}$'),
    display_name text NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 128),
    role text NOT NULL CHECK (role IN ('admin', 'operator', 'viewer')),
    token_digest bytea NOT NULL UNIQUE CHECK (octet_length(token_digest) = 32),
    created_by uuid NOT NULL REFERENCES users(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    accepted_at timestamptz,
    declined_at timestamptz,
    PRIMARY KEY (tenant_id, id),
    CHECK (expires_at > created_at),
    CHECK (accepted_at IS NULL OR declined_at IS NULL)
);

ALTER TABLE member_invitations ENABLE ROW LEVEL SECURITY;
ALTER TABLE member_invitations FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON member_invitations
    USING (tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid);

CREATE INDEX member_invitations_pending_idx
    ON member_invitations (tenant_id, created_at DESC)
    WHERE accepted_at IS NULL AND declined_at IS NULL;
