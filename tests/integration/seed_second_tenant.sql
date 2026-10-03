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

-- Exercise every least-privilege role against the real Community tenant. Product-created
-- invitations will replace this test-only fixture; no plaintext password is stored here.
INSERT INTO users (id, email, display_name, local_password_hash)
SELECT fixture.id, fixture.email, fixture.display_name, owner.local_password_hash
FROM users AS owner
CROSS JOIN (
    VALUES
        ('33333333-3333-4333-8333-333333333331'::uuid, 'admin@example.com', 'Test Admin'),
        ('33333333-3333-4333-8333-333333333332'::uuid, 'operator@example.com', 'Test Operator'),
        ('33333333-3333-4333-8333-333333333333'::uuid, 'viewer@example.com', 'Test Viewer')
) AS fixture(id, email, display_name)
WHERE owner.email = 'e2e-owner@example.com';

INSERT INTO memberships (tenant_id, user_id, role)
SELECT system_state.bootstrap_tenant_id, fixture.user_id, fixture.role
FROM system_state
CROSS JOIN (
    VALUES
        ('33333333-3333-4333-8333-333333333331'::uuid, 'admin'),
        ('33333333-3333-4333-8333-333333333332'::uuid, 'operator'),
        ('33333333-3333-4333-8333-333333333333'::uuid, 'viewer')
) AS fixture(user_id, role)
WHERE system_state.singleton_id = 1;
