-- Monitoring Alpha: one explicit scheduled network source per service. Claims cross the
-- forced-RLS boundary only through a narrowly granted lease function; results are written after
-- restoring the tenant context in the application transaction.
CREATE TABLE service_probes (
    tenant_id uuid NOT NULL,
    id uuid NOT NULL DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL,
    service_id uuid NOT NULL,
    kind text NOT NULL CHECK (kind IN ('http', 'https', 'tcp', 'dns', 'tls')),
    host text NOT NULL CHECK (char_length(host) BETWEEN 1 AND 253),
    port integer CHECK (port BETWEEN 1 AND 65535),
    path text CHECK (path IS NULL OR char_length(path) BETWEEN 1 AND 2048),
    expected_status integer CHECK (expected_status BETWEEN 100 AND 599),
    timeout_ms integer NOT NULL DEFAULT 5000 CHECK (timeout_ms BETWEEN 250 AND 10000),
    interval_seconds integer NOT NULL DEFAULT 15 CHECK (interval_seconds BETWEEN 10 AND 86400),
    failure_threshold integer NOT NULL DEFAULT 2 CHECK (failure_threshold BETWEEN 1 AND 5),
    recovery_threshold integer NOT NULL DEFAULT 1 CHECK (recovery_threshold BETWEEN 1 AND 5),
    enabled boolean NOT NULL DEFAULT true,
    next_run_at timestamptz NOT NULL DEFAULT now(),
    locked_at timestamptz,
    locked_by uuid,
    consecutive_failures integer NOT NULL DEFAULT 0 CHECK (consecutive_failures >= 0),
    consecutive_successes integer NOT NULL DEFAULT 0 CHECK (consecutive_successes >= 0),
    consensus_state text NOT NULL DEFAULT 'unknown'
        CHECK (consensus_state IN ('unknown', 'online', 'offline')),
    last_state text CHECK (last_state IS NULL OR last_state IN (
        'responding', 'unexpected_status', 'could_not_resolve', 'no_response', 'timed_out',
        'invalid_certificate', 'unsafe_destination', 'invalid_request'
    )),
    last_message text CHECK (last_message IS NULL OR char_length(last_message) <= 512),
    last_observed_at timestamptz,
    last_response_ms bigint CHECK (last_response_ms IS NULL OR last_response_ms >= 0),
    last_status_code integer CHECK (last_status_code IS NULL OR last_status_code BETWEEN 100 AND 599),
    last_tls_expires_at timestamptz,
    created_by uuid NOT NULL REFERENCES users(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, id),
    UNIQUE (tenant_id, service_id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, service_id) REFERENCES services(tenant_id, id) ON DELETE CASCADE,
    CHECK ((kind = 'tcp') = (port IS NOT NULL) OR kind <> 'tcp'),
    CHECK (kind <> 'dns' OR (port IS NULL AND path IS NULL AND expected_status IS NULL)),
    CHECK (kind IN ('http', 'https') OR expected_status IS NULL),
    CHECK ((locked_at IS NULL) = (locked_by IS NULL))
);

CREATE INDEX service_probes_due_idx
ON service_probes (next_run_at, id)
WHERE enabled;

CREATE TABLE probe_observations (
    tenant_id uuid NOT NULL,
    id uuid NOT NULL DEFAULT gen_random_uuid(),
    probe_id uuid NOT NULL,
    service_id uuid NOT NULL,
    state text NOT NULL CHECK (state IN (
        'responding', 'unexpected_status', 'could_not_resolve', 'no_response', 'timed_out',
        'invalid_certificate', 'unsafe_destination', 'invalid_request'
    )),
    message text NOT NULL CHECK (char_length(message) BETWEEN 1 AND 512),
    observed_at timestamptz NOT NULL,
    response_ms bigint CHECK (response_ms IS NULL OR response_ms >= 0),
    status_code integer CHECK (status_code IS NULL OR status_code BETWEEN 100 AND 599),
    resolved_addresses integer NOT NULL CHECK (resolved_addresses BETWEEN 0 AND 32),
    tls_expires_at timestamptz,
    tls_days_remaining bigint,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, id),
    FOREIGN KEY (tenant_id, probe_id) REFERENCES service_probes(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, service_id) REFERENCES services(tenant_id, id) ON DELETE CASCADE
);

CREATE INDEX probe_observations_timeline_idx
ON probe_observations (tenant_id, probe_id, observed_at DESC, id);

ALTER TABLE service_probes ENABLE ROW LEVEL SECURITY;
ALTER TABLE service_probes FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON service_probes
    USING (tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid);

ALTER TABLE probe_observations ENABLE ROW LEVEL SECURITY;
ALTER TABLE probe_observations FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON probe_observations
    USING (tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid);

CREATE FUNCTION meerkateer_claim_service_probes(p_worker_id uuid, p_limit integer)
RETURNS TABLE (
    probe_id uuid,
    tenant_id uuid,
    project_id uuid,
    service_id uuid,
    kind text,
    host text,
    port integer,
    path text,
    expected_status integer,
    timeout_ms integer,
    interval_seconds integer,
    failure_threshold integer,
    recovery_threshold integer,
    service_slug text,
    project_slug text,
    environment text
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
BEGIN
    IF p_worker_id IS NULL OR p_limit NOT BETWEEN 1 AND 50 THEN
        RAISE EXCEPTION 'invalid scheduled probe claim' USING ERRCODE = '22023';
    END IF;
    RETURN QUERY
    WITH due AS (
        SELECT probes.tenant_id, probes.id
        FROM public.service_probes AS probes
        WHERE probes.enabled
          AND probes.next_run_at <= clock_timestamp()
          AND (probes.locked_at IS NULL OR probes.locked_at < clock_timestamp() - interval '30 seconds')
        ORDER BY probes.next_run_at, probes.id
        FOR UPDATE SKIP LOCKED
        LIMIT p_limit
    ), claimed AS (
        UPDATE public.service_probes AS probes
        SET locked_at = clock_timestamp(), locked_by = p_worker_id, updated_at = clock_timestamp()
        FROM due
        WHERE probes.tenant_id = due.tenant_id AND probes.id = due.id
        RETURNING probes.*
    )
    SELECT claimed.id, claimed.tenant_id, claimed.project_id, claimed.service_id,
           claimed.kind, claimed.host, claimed.port, claimed.path, claimed.expected_status,
           claimed.timeout_ms, claimed.interval_seconds, claimed.failure_threshold,
           claimed.recovery_threshold, services.slug, projects.slug, services.environment
    FROM claimed
    JOIN public.services ON services.tenant_id = claimed.tenant_id
      AND services.id = claimed.service_id
    JOIN public.projects ON projects.tenant_id = claimed.tenant_id
      AND projects.id = claimed.project_id;
END;
$$;

REVOKE ALL ON FUNCTION meerkateer_claim_service_probes(uuid, integer) FROM PUBLIC;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'meerkateer_app') THEN
        GRANT SELECT, INSERT, UPDATE, DELETE ON service_probes, probe_observations TO meerkateer_app;
        GRANT EXECUTE ON FUNCTION meerkateer_claim_service_probes(uuid, integer) TO meerkateer_app;
    END IF;
END;
$$;
