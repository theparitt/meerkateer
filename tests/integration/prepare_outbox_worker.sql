\set ON_ERROR_STOP on

-- One record must be scheduled for retry and one must reach the DLQ on this cycle.
-- The test owner inserts these directly so no unsupported public API is required.
INSERT INTO outbox (tenant_id, topic, payload)
SELECT id, 'test.retry', '{"case":"retry"}'::jsonb
FROM tenants
WHERE slug = 'e2e-operator';

INSERT INTO outbox (tenant_id, topic, payload, attempts)
SELECT id, 'test.dead-letter', '{"case":"dead-letter"}'::jsonb, 4
FROM tenants
WHERE slug = 'e2e-operator';
