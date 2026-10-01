-- First-class operational records. Incidents and alert deliveries are durable facts;
-- maintenance windows suppress notifications, never the underlying health evidence.
CREATE TABLE incidents (
    tenant_id uuid NOT NULL,
    id uuid NOT NULL DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL,
    service_id uuid NOT NULL,
    status text NOT NULL CHECK (status IN ('open', 'resolved')),
    severity text NOT NULL CHECK (severity IN ('warning', 'critical')),
    title text NOT NULL CHECK (char_length(title) BETWEEN 1 AND 256),
    cause text NOT NULL CHECK (char_length(cause) BETWEEN 1 AND 1024),
    started_at timestamptz NOT NULL,
    last_observed_at timestamptz NOT NULL,
    resolved_at timestamptz,
    opened_by_ingest_key uuid NOT NULL,
    resolved_by_ingest_key uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, service_id) REFERENCES services(tenant_id, id) ON DELETE CASCADE,
    CHECK ((status = 'open' AND resolved_at IS NULL AND resolved_by_ingest_key IS NULL)
        OR (status = 'resolved' AND resolved_at IS NOT NULL AND resolved_by_ingest_key IS NOT NULL)),
    CHECK (last_observed_at >= started_at),
    CHECK (resolved_at IS NULL OR resolved_at >= started_at)
);

CREATE UNIQUE INDEX incidents_one_open_per_service_idx
ON incidents (tenant_id, service_id) WHERE status = 'open';
CREATE INDEX incidents_project_timeline_idx
ON incidents (tenant_id, project_id, started_at DESC, id);

CREATE TABLE incident_events (
    tenant_id uuid NOT NULL,
    incident_id uuid NOT NULL,
    ingest_key uuid NOT NULL,
    kind text NOT NULL CHECK (kind IN ('opened', 'resolved')),
    state text NOT NULL CHECK (state IN ('offline', 'online')),
    observed_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, incident_id, ingest_key),
    FOREIGN KEY (tenant_id, incident_id) REFERENCES incidents(tenant_id, id) ON DELETE CASCADE
);

CREATE TABLE maintenance_windows (
    tenant_id uuid NOT NULL,
    id uuid NOT NULL DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL,
    service_id uuid NOT NULL,
    title text NOT NULL CHECK (char_length(title) BETWEEN 1 AND 128),
    reason text NOT NULL CHECK (char_length(reason) BETWEEN 1 AND 1024),
    starts_at timestamptz NOT NULL,
    ends_at timestamptz NOT NULL,
    created_by uuid NOT NULL REFERENCES users(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    cancelled_at timestamptz,
    PRIMARY KEY (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, service_id) REFERENCES services(tenant_id, id) ON DELETE CASCADE,
    CHECK (ends_at > starts_at),
    CHECK (ends_at <= starts_at + interval '90 days')
);

CREATE INDEX maintenance_windows_service_time_idx
ON maintenance_windows (tenant_id, service_id, starts_at, ends_at)
WHERE cancelled_at IS NULL;

CREATE TABLE alert_policies (
    tenant_id uuid PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    enabled boolean NOT NULL DEFAULT true,
    notify_down boolean NOT NULL DEFAULT true,
    notify_recovered boolean NOT NULL DEFAULT true,
    updated_by uuid NOT NULL REFERENCES users(id),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE alert_deliveries (
    tenant_id uuid NOT NULL,
    id uuid NOT NULL DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL,
    service_id uuid NOT NULL,
    incident_id uuid,
    outbox_id uuid UNIQUE REFERENCES outbox(id) ON DELETE RESTRICT,
    transition text NOT NULL CHECK (transition IN ('down', 'recovered')),
    status text NOT NULL CHECK (status IN (
        'queued', 'retrying', 'delivered', 'dead_lettered',
        'suppressed', 'skipped_disabled', 'skipped_unconfigured'
    )),
    observed_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    delivered_at timestamptz,
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_error text CHECK (last_error IS NULL OR char_length(last_error) <= 2048),
    suppression_reason text CHECK (
        suppression_reason IS NULL OR char_length(suppression_reason) BETWEEN 1 AND 1024
    ),
    PRIMARY KEY (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, service_id) REFERENCES services(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, incident_id) REFERENCES incidents(tenant_id, id)
        ON DELETE SET NULL (incident_id),
    CHECK ((status IN ('queued', 'retrying', 'delivered', 'dead_lettered')) = (outbox_id IS NOT NULL)),
    CHECK ((status = 'delivered') = (delivered_at IS NOT NULL))
);

CREATE INDEX alert_deliveries_project_timeline_idx
ON alert_deliveries (tenant_id, project_id, created_at DESC, id);

DO $$
DECLARE
    table_name text;
BEGIN
    FOREACH table_name IN ARRAY ARRAY[
        'incidents', 'incident_events', 'maintenance_windows', 'alert_policies', 'alert_deliveries'
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

-- Keep the operator-visible delivery record synchronized with the durable outbox lease.
CREATE OR REPLACE FUNCTION meerkateer_complete_outbox(p_worker_id uuid, p_outbox_id uuid)
RETURNS boolean
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
DECLARE
    completed_attempts integer;
BEGIN
    UPDATE public.outbox
    SET processed_at = clock_timestamp(), locked_at = NULL, locked_by = NULL, last_error = NULL
    WHERE id = p_outbox_id AND locked_by = p_worker_id AND processed_at IS NULL
    RETURNING attempts INTO completed_attempts;
    IF completed_attempts IS NULL THEN
        RETURN false;
    END IF;
    UPDATE public.alert_deliveries
    SET status = 'delivered', delivered_at = clock_timestamp(), attempts = completed_attempts,
        last_error = NULL
    WHERE outbox_id = p_outbox_id;
    RETURN true;
END;
$$;

CREATE OR REPLACE FUNCTION meerkateer_retry_outbox(
    p_worker_id uuid, p_outbox_id uuid, p_error text, p_delay_seconds integer
)
RETURNS boolean
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
DECLARE
    retry_attempts integer;
BEGIN
    IF p_delay_seconds NOT BETWEEN 1 AND 3600 THEN
        RAISE EXCEPTION 'outbox retry delay must be between 1 and 3600 seconds' USING ERRCODE = '22023';
    END IF;
    IF p_error IS NULL OR btrim(p_error) = '' THEN
        RAISE EXCEPTION 'outbox retry error must not be empty' USING ERRCODE = '22023';
    END IF;
    UPDATE public.outbox
    SET available_at = clock_timestamp() + make_interval(secs => p_delay_seconds),
        locked_at = NULL, locked_by = NULL, last_error = left(p_error, 2048)
    WHERE id = p_outbox_id AND locked_by = p_worker_id AND processed_at IS NULL
    RETURNING attempts INTO retry_attempts;
    IF retry_attempts IS NULL THEN
        RETURN false;
    END IF;
    UPDATE public.alert_deliveries
    SET status = 'retrying', attempts = retry_attempts, last_error = left(p_error, 2048)
    WHERE outbox_id = p_outbox_id;
    RETURN true;
END;
$$;

CREATE OR REPLACE FUNCTION meerkateer_dead_letter_outbox(
    p_worker_id uuid, p_outbox_id uuid, p_error text
)
RETURNS boolean
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
DECLARE
    terminal public.outbox%ROWTYPE;
BEGIN
    IF p_error IS NULL OR btrim(p_error) = '' THEN
        RAISE EXCEPTION 'dead-letter error must not be empty';
    END IF;
    UPDATE public.outbox
    SET processed_at = clock_timestamp(), dead_lettered_at = clock_timestamp(),
        locked_at = NULL, locked_by = NULL, last_error = left(p_error, 2048)
    WHERE id = p_outbox_id AND locked_by = p_worker_id AND processed_at IS NULL
    RETURNING * INTO terminal;
    IF terminal.id IS NULL THEN
        RETURN false;
    END IF;
    INSERT INTO public.outbox_dead_letters (
        tenant_id, outbox_id, topic, payload, attempts, last_error, failed_at
    ) VALUES (
        terminal.tenant_id, terminal.id, terminal.topic, terminal.payload,
        terminal.attempts, terminal.last_error, terminal.dead_lettered_at
    ) ON CONFLICT (outbox_id) DO NOTHING;
    UPDATE public.alert_deliveries
    SET status = 'dead_lettered', attempts = terminal.attempts, last_error = terminal.last_error
    WHERE outbox_id = p_outbox_id;
    RETURN true;
END;
$$;

REVOKE ALL ON FUNCTION meerkateer_complete_outbox(uuid, uuid) FROM PUBLIC;
REVOKE ALL ON FUNCTION meerkateer_retry_outbox(uuid, uuid, text, integer) FROM PUBLIC;
REVOKE ALL ON FUNCTION meerkateer_dead_letter_outbox(uuid, uuid, text) FROM PUBLIC;

-- Existing installations already have this role; fresh installations create it later.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'meerkateer_outbox_executor') THEN
        GRANT SELECT, UPDATE ON alert_deliveries TO meerkateer_outbox_executor;
    END IF;
END;
$$;
