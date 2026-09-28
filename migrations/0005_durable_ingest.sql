-- Accepted MKS-1 messages are immutable, idempotent facts. Current status and the
-- downstream outbox are updated in the same transaction as this durable record.
CREATE TABLE ingest_messages (
    tenant_id uuid NOT NULL,
    service_id uuid NOT NULL,
    idempotency_key uuid NOT NULL,
    message_kind text NOT NULL CHECK (message_kind IN ('heartbeat', 'event', 'deploy')),
    observed_at timestamptz NOT NULL,
    payload jsonb NOT NULL CHECK (
        jsonb_typeof(payload) = 'object' AND pg_column_size(payload) <= 32768
    ),
    received_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, service_id, idempotency_key),
    FOREIGN KEY (tenant_id, service_id) REFERENCES services(tenant_id, id) ON DELETE CASCADE
);

CREATE INDEX ingest_messages_timeline_idx
ON ingest_messages (tenant_id, service_id, observed_at DESC, idempotency_key);

ALTER TABLE ingest_messages ENABLE ROW LEVEL SECURITY;
ALTER TABLE ingest_messages FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON ingest_messages
    USING (tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid);
