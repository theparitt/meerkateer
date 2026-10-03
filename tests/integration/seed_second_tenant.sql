\set ON_ERROR_STOP on

-- The API exposes one-company Community setup, but the shared core is tenant-aware for Cloud.
-- Seed a second company only inside this disposable security exercise. Reusing the already-hashed
-- test password avoids placing another password or implementation-specific Argon2 fixture here.
INSERT INTO tenants (id, slug, display_name)
VALUES (
    '22222222-2222-4222-8222-222222222222',
    'isolation-rival',
    'Isolation Rival'
);

INSERT INTO users (id, email, display_name, local_password_hash)
SELECT
    '22222222-2222-4222-8222-222222222223',
    'isolation-owner@example.com',
    'Isolation Owner',
    local_password_hash
FROM users
WHERE email = 'e2e-owner@example.com';

INSERT INTO memberships (tenant_id, user_id, role)
VALUES (
    '22222222-2222-4222-8222-222222222222',
    '22222222-2222-4222-8222-222222222223',
    'owner'
);

INSERT INTO sessions (
    id,
    tenant_id,
    user_id,
    token_digest,
    csrf_digest,
    expires_at
)
VALUES (
    '22222222-2222-4222-8222-222222222224',
    '22222222-2222-4222-8222-222222222222',
    '22222222-2222-4222-8222-222222222223',
    digest(
        'mks_session_22222222222242228222222222222222_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
        'sha256'
    ),
    digest(
        'mks_csrf_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
        'sha256'
    ),
    now() + interval '1 hour'
);
