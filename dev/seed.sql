SELECT set_config('meerkateer.tenant_id', '00000000-0000-4000-8000-000000000001', false);

INSERT INTO tenants (id, slug, display_name)
VALUES ('00000000-0000-4000-8000-000000000001', 'local-operator', 'Local Operator')
ON CONFLICT DO NOTHING;

INSERT INTO projects (tenant_id, id, slug, display_name)
VALUES (
    '00000000-0000-4000-8000-000000000001',
    '00000000-0000-4000-8000-000000000002',
    'game-lab',
    'Game Server Lab'
)
ON CONFLICT DO NOTHING;

INSERT INTO services (tenant_id, project_id, id, slug, environment)
VALUES (
    '00000000-0000-4000-8000-000000000001',
    '00000000-0000-4000-8000-000000000002',
    '00000000-0000-4000-8000-000000000003',
    'fixture-server',
    'local'
)
ON CONFLICT DO NOTHING;
