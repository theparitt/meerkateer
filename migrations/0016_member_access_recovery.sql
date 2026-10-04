ALTER TABLE member_invitations
    ADD COLUMN cancelled_at timestamptz;

ALTER TABLE member_invitations
    DROP CONSTRAINT IF EXISTS member_invitations_terminal_state_check;
ALTER TABLE member_invitations
    ADD CONSTRAINT member_invitations_terminal_state_check CHECK (
        ((accepted_at IS NOT NULL)::integer
         + (declined_at IS NOT NULL)::integer
         + (cancelled_at IS NOT NULL)::integer) <= 1
    );

DROP INDEX IF EXISTS member_invitations_pending_idx;
CREATE INDEX member_invitations_pending_idx
    ON member_invitations (tenant_id, created_at DESC)
    WHERE accepted_at IS NULL AND declined_at IS NULL AND cancelled_at IS NULL;

CREATE TABLE member_password_resets (
    tenant_id uuid NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    token_digest bytea NOT NULL UNIQUE CHECK (octet_length(token_digest) = 32),
    created_by uuid NOT NULL REFERENCES users(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    consumed_at timestamptz,
    cancelled_at timestamptz,
    PRIMARY KEY (tenant_id, id),
    FOREIGN KEY (tenant_id, user_id)
        REFERENCES memberships(tenant_id, user_id) ON DELETE CASCADE,
    CHECK (expires_at > created_at),
    CHECK (consumed_at IS NULL OR cancelled_at IS NULL)
);

ALTER TABLE member_password_resets ENABLE ROW LEVEL SECURITY;
ALTER TABLE member_password_resets FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON member_password_resets
    USING (tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid);

CREATE INDEX member_password_resets_pending_idx
    ON member_password_resets (tenant_id, user_id, created_at DESC)
    WHERE consumed_at IS NULL AND cancelled_at IS NULL;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'meerkateer_app') THEN
        GRANT SELECT, INSERT, UPDATE, DELETE ON public.member_password_resets TO meerkateer_app;
    END IF;
END;
$$;
