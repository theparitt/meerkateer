CREATE TABLE users (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    email text NOT NULL CHECK (char_length(email) BETWEEN 3 AND 254 AND email = lower(email)),
    display_name text NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 128),
    disabled_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (email)
);

CREATE TABLE memberships (
    tenant_id uuid NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role text NOT NULL CHECK (role IN ('owner', 'admin', 'operator', 'viewer')),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, user_id)
);

CREATE TABLE oidc_identities (
    issuer text NOT NULL CHECK (char_length(issuer) BETWEEN 8 AND 2048),
    subject text NOT NULL CHECK (char_length(subject) BETWEEN 1 AND 255),
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (issuer, subject),
    UNIQUE (user_id, issuer)
);

CREATE TABLE sessions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id uuid NOT NULL,
    token_digest bytea NOT NULL UNIQUE CHECK (octet_length(token_digest) = 32),
    csrf_digest bytea NOT NULL CHECK (octet_length(csrf_digest) = 32),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (tenant_id, user_id) REFERENCES memberships(tenant_id, user_id) ON DELETE CASCADE,
    CHECK (expires_at > created_at)
);

CREATE TABLE service_credentials (
    tenant_id uuid NOT NULL,
    service_id uuid NOT NULL,
    id uuid NOT NULL,
    prefix text NOT NULL CHECK (prefix ~ '^[0-9a-f]{12}$'),
    password_hash text NOT NULL CHECK (password_hash LIKE '$argon2id$%'),
    created_by uuid NOT NULL REFERENCES users(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    valid_after timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz,
    revoked_at timestamptz,
    last_used_at timestamptz,
    PRIMARY KEY (tenant_id, id),
    UNIQUE (prefix),
    FOREIGN KEY (tenant_id, service_id) REFERENCES services(tenant_id, id) ON DELETE CASCADE,
    CHECK (expires_at IS NULL OR expires_at > valid_after)
);

CREATE TABLE enrollment_tokens (
    tenant_id uuid NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    id uuid NOT NULL,
    prefix text NOT NULL CHECK (prefix ~ '^[0-9a-f]{12}$'),
    password_hash text NOT NULL CHECK (password_hash LIKE '$argon2id$%'),
    expires_at timestamptz NOT NULL,
    consumed_at timestamptz,
    created_by uuid NOT NULL REFERENCES users(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, id),
    UNIQUE (prefix),
    CHECK (expires_at > created_at)
);

CREATE TABLE agents (
    tenant_id uuid NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    id uuid NOT NULL,
    machine_fingerprint_digest bytea NOT NULL CHECK (octet_length(machine_fingerprint_digest) = 32),
    display_name text NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 128),
    status text NOT NULL CHECK (status IN ('active', 'revoked', 'quarantined')),
    enrolled_at timestamptz NOT NULL DEFAULT now(),
    revoked_at timestamptz,
    last_seen_at timestamptz,
    PRIMARY KEY (tenant_id, id),
    UNIQUE (tenant_id, machine_fingerprint_digest)
);

CREATE TABLE agent_credentials (
    tenant_id uuid NOT NULL,
    agent_id uuid NOT NULL,
    id uuid NOT NULL,
    prefix text NOT NULL CHECK (prefix ~ '^[0-9a-f]{12}$'),
    password_hash text NOT NULL CHECK (password_hash LIKE '$argon2id$%'),
    valid_after timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, id),
    UNIQUE (prefix),
    FOREIGN KEY (tenant_id, agent_id) REFERENCES agents(tenant_id, id) ON DELETE CASCADE,
    CHECK (expires_at > valid_after)
);

CREATE TABLE desired_agent_configs (
    tenant_id uuid NOT NULL,
    agent_id uuid NOT NULL,
    version bigint NOT NULL CHECK (version > 0),
    body jsonb NOT NULL CHECK (jsonb_typeof(body) = 'object'),
    signature bytea NOT NULL CHECK (octet_length(signature) BETWEEN 32 AND 512),
    key_id text NOT NULL CHECK (char_length(key_id) BETWEEN 1 AND 64),
    created_by uuid NOT NULL REFERENCES users(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, agent_id, version),
    FOREIGN KEY (tenant_id, agent_id) REFERENCES agents(tenant_id, id) ON DELETE CASCADE
);

CREATE TABLE audit_events (
    tenant_id uuid NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    id uuid NOT NULL DEFAULT gen_random_uuid(),
    actor_type text NOT NULL CHECK (actor_type IN ('user', 'service', 'agent', 'system')),
    actor_id uuid,
    action text NOT NULL CHECK (char_length(action) BETWEEN 3 AND 128),
    target_type text NOT NULL CHECK (char_length(target_type) BETWEEN 1 AND 64),
    target_id uuid,
    request_id uuid,
    source_ip_digest bytea CHECK (source_ip_digest IS NULL OR octet_length(source_ip_digest) = 32),
    details jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(details) = 'object'),
    occurred_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, id)
);

CREATE OR REPLACE FUNCTION reject_audit_mutation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'audit events are append-only';
END;
$$;

CREATE TRIGGER audit_events_no_update
BEFORE UPDATE OR DELETE ON audit_events
FOR EACH ROW EXECUTE FUNCTION reject_audit_mutation();

ALTER TABLE tenants ENABLE ROW LEVEL SECURITY;
ALTER TABLE tenants FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON tenants
    USING (id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid)
    WITH CHECK (id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid);

ALTER TABLE users ENABLE ROW LEVEL SECURITY;
ALTER TABLE users FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_user_isolation ON users
    USING (EXISTS (
        SELECT 1 FROM memberships
        WHERE memberships.user_id = users.id
          AND memberships.tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid
    ))
    WITH CHECK (true);

ALTER TABLE oidc_identities ENABLE ROW LEVEL SECURITY;
ALTER TABLE oidc_identities FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_oidc_isolation ON oidc_identities
    USING (EXISTS (
        SELECT 1 FROM memberships
        WHERE memberships.user_id = oidc_identities.user_id
          AND memberships.tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid
    ))
    WITH CHECK (true);

DO $$
DECLARE
    table_name text;
BEGIN
    FOREACH table_name IN ARRAY ARRAY[
        'projects', 'services', 'service_snapshots', 'outbox', 'telemetry_samples',
        'memberships', 'sessions', 'service_credentials', 'enrollment_tokens', 'agents',
        'agent_credentials', 'desired_agent_configs', 'audit_events'
    ]
    LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', table_name);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', table_name);
        EXECUTE format(
            'CREATE POLICY tenant_isolation ON %I USING '
            || '(tenant_id = nullif(current_setting(''meerkateer.tenant_id'', true), '''')::uuid) '
            || 'WITH CHECK '
            || '(tenant_id = nullif(current_setting(''meerkateer.tenant_id'', true), '''')::uuid)',
            table_name
        );
    END LOOP;
END;
$$;

CREATE INDEX sessions_active_digest_idx ON sessions (token_digest)
WHERE revoked_at IS NULL;
CREATE INDEX service_credentials_active_prefix_idx ON service_credentials (prefix)
WHERE revoked_at IS NULL;
CREATE INDEX agent_credentials_active_prefix_idx ON agent_credentials (prefix)
WHERE revoked_at IS NULL;
CREATE INDEX audit_events_timeline_idx ON audit_events (tenant_id, occurred_at DESC, id);
