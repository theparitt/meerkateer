-- Phase 1 control-plane foundation. All tenant-owned foreign keys include tenant_id so
-- later repository queries can enforce the boundary structurally.
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE tenants (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    slug text NOT NULL UNIQUE CHECK (slug ~ '^[a-z0-9][a-z0-9_-]{0,62}[a-z0-9]$'),
    display_name text NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 128),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE projects (
    tenant_id uuid NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    id uuid NOT NULL DEFAULT gen_random_uuid(),
    slug text NOT NULL CHECK (slug ~ '^[a-z0-9][a-z0-9_-]{0,62}[a-z0-9]$'),
    display_name text NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 128),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, id),
    UNIQUE (tenant_id, slug)
);

CREATE TABLE services (
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    id uuid NOT NULL DEFAULT gen_random_uuid(),
    slug text NOT NULL CHECK (slug ~ '^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$'),
    environment text NOT NULL CHECK (environment IN ('development', 'staging', 'production', 'test', 'local')),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, id),
    UNIQUE (tenant_id, project_id, slug, environment),
    FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE
);

CREATE TABLE service_snapshots (
    tenant_id uuid NOT NULL,
    service_id uuid NOT NULL,
    state text NOT NULL CHECK (state IN ('online', 'degraded', 'offline', 'maintenance', 'unknown')),
    last_sequence bigint NOT NULL CHECK (last_sequence >= 0),
    observed_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, service_id),
    FOREIGN KEY (tenant_id, service_id) REFERENCES services(tenant_id, id) ON DELETE CASCADE
);

CREATE TABLE outbox (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    topic text NOT NULL CHECK (char_length(topic) BETWEEN 1 AND 128),
    payload jsonb NOT NULL CHECK (jsonb_typeof(payload) = 'object'),
    created_at timestamptz NOT NULL DEFAULT now(),
    available_at timestamptz NOT NULL DEFAULT now(),
    processed_at timestamptz,
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0)
);

CREATE INDEX outbox_pending_idx ON outbox (available_at, created_at)
WHERE processed_at IS NULL;

CREATE TABLE telemetry_samples (
    tenant_id uuid NOT NULL,
    service_id uuid NOT NULL,
    sequence bigint NOT NULL CHECK (sequence > 0),
    metric text NOT NULL CHECK (metric ~ '^[a-z][a-z0-9_.]{0,127}$'),
    value double precision NOT NULL CHECK (
        value NOT IN ('NaN'::double precision, 'Infinity'::double precision, '-Infinity'::double precision)
    ),
    observed_at timestamptz NOT NULL,
    PRIMARY KEY (tenant_id, service_id, sequence, metric),
    FOREIGN KEY (tenant_id, service_id) REFERENCES services(tenant_id, id) ON DELETE CASCADE
);

CREATE INDEX telemetry_samples_time_idx
ON telemetry_samples (tenant_id, service_id, observed_at DESC);
