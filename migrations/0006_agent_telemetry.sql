CREATE TABLE agent_sequence_state (
    tenant_id uuid NOT NULL,
    agent_id uuid NOT NULL,
    last_sequence bigint NOT NULL CHECK (last_sequence >= 0),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, agent_id),
    FOREIGN KEY (tenant_id, agent_id) REFERENCES agents(tenant_id, id) ON DELETE CASCADE
);

CREATE TABLE agent_batches (
    tenant_id uuid NOT NULL,
    agent_id uuid NOT NULL,
    batch_id uuid NOT NULL,
    first_sequence bigint NOT NULL CHECK (first_sequence > 0),
    last_sequence bigint NOT NULL CHECK (last_sequence >= first_sequence),
    payload_digest bytea NOT NULL CHECK (octet_length(payload_digest) = 32),
    gap_detected boolean NOT NULL,
    received_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, agent_id, batch_id),
    FOREIGN KEY (tenant_id, agent_id) REFERENCES agents(tenant_id, id) ON DELETE CASCADE
);

CREATE TABLE agent_telemetry_records (
    tenant_id uuid NOT NULL,
    agent_id uuid NOT NULL,
    record_id uuid NOT NULL,
    sequence bigint NOT NULL CHECK (sequence > 0),
    record_type text NOT NULL CHECK (record_type IN ('sample', 'event')),
    observed_at timestamptz NOT NULL,
    name text NOT NULL CHECK (char_length(name) BETWEEN 2 AND 128),
    value double precision CHECK (
        value IS NULL OR value NOT IN (
            'NaN'::double precision,
            'Infinity'::double precision,
            '-Infinity'::double precision
        )
    ),
    severity text CHECK (severity IS NULL OR severity IN ('info', 'warning', 'error', 'critical')),
    message text CHECK (message IS NULL OR char_length(message) <= 1024),
    attributes jsonb NOT NULL CHECK (
        jsonb_typeof(attributes) = 'object' AND pg_column_size(attributes) <= 4096
    ),
    PRIMARY KEY (tenant_id, agent_id, record_id),
    UNIQUE (tenant_id, agent_id, sequence),
    FOREIGN KEY (tenant_id, agent_id) REFERENCES agents(tenant_id, id) ON DELETE CASCADE,
    CHECK (
        (record_type = 'sample' AND value IS NOT NULL AND severity IS NULL AND message IS NULL)
        OR (record_type = 'event' AND value IS NULL AND severity IS NOT NULL)
    )
);

CREATE INDEX agent_telemetry_timeline_idx
ON agent_telemetry_records (tenant_id, agent_id, observed_at DESC, sequence DESC);

DO $$
DECLARE
    table_name text;
BEGIN
    FOREACH table_name IN ARRAY ARRAY[
        'agent_sequence_state', 'agent_batches', 'agent_telemetry_records'
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
