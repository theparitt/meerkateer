-- Security Beta foundation: a replica-wide, pseudonymous fixed-window limiter. The application
-- role can consume a bucket only through the bounded SECURITY DEFINER function and has no direct
-- access to peer digests.
BEGIN;

CREATE TABLE distributed_rate_limits (
    scope text NOT NULL CHECK (scope IN (
        'authentication', 'invitation_acceptance', 'password_reset_acceptance',
        'password_login', 'game_probe', 'network_probe', 'ingestion'
    )),
    peer_digest bytea NOT NULL CHECK (octet_length(peer_digest) = 32),
    window_started_at timestamptz NOT NULL,
    -- One value beyond the public maximum lets a saturated bucket keep returning false without
    -- overflowing or turning a normal rejection into a database error.
    request_count integer NOT NULL CHECK (request_count BETWEEN 1 AND 100001),
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (scope, peer_digest)
);

CREATE INDEX distributed_rate_limits_expiry_idx
ON distributed_rate_limits (updated_at);

REVOKE ALL ON distributed_rate_limits FROM PUBLIC;

CREATE FUNCTION meerkateer_consume_rate_limit(
    p_scope text,
    p_peer_digest bytea,
    p_limit integer
)
RETURNS boolean
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
DECLARE
    current_window timestamptz := date_trunc('minute', clock_timestamp());
    consumed integer;
BEGIN
    IF p_scope IS NULL OR p_peer_digest IS NULL OR p_limit IS NULL OR p_scope NOT IN (
        'authentication', 'invitation_acceptance', 'password_reset_acceptance',
        'password_login', 'game_probe', 'network_probe', 'ingestion'
    ) OR octet_length(p_peer_digest) <> 32 OR p_limit NOT BETWEEN 1 AND 100000 THEN
        RAISE EXCEPTION 'invalid distributed rate-limit request' USING ERRCODE = '22023';
    END IF;

    -- Keep ten minutes of pseudonymous buckets. The indexed delete is idempotent and prevents
    -- abandoned peers from growing this security table indefinitely across replicas.
    DELETE FROM public.distributed_rate_limits
    WHERE updated_at < clock_timestamp() - interval '10 minutes';

    IF NOT EXISTS (
        SELECT 1 FROM public.distributed_rate_limits
        WHERE scope = p_scope AND peer_digest = p_peer_digest
    ) THEN
        -- Serialize only new-bucket capacity checks. Existing hot peers remain row-local.
        PERFORM pg_advisory_xact_lock(714032145);
        IF (SELECT count(*) FROM public.distributed_rate_limits) >= 65536 THEN
            RETURN false;
        END IF;
    END IF;

    INSERT INTO public.distributed_rate_limits (
        scope, peer_digest, window_started_at, request_count, updated_at
    ) VALUES (
        p_scope, p_peer_digest, current_window, 1, clock_timestamp()
    )
    ON CONFLICT (scope, peer_digest) DO UPDATE SET
        window_started_at = CASE
            WHEN distributed_rate_limits.window_started_at = current_window
                THEN distributed_rate_limits.window_started_at
            ELSE current_window
        END,
        request_count = CASE
            WHEN distributed_rate_limits.window_started_at = current_window
                THEN LEAST(distributed_rate_limits.request_count + 1, 100001)
            ELSE 1
        END,
        updated_at = clock_timestamp()
    RETURNING request_count INTO consumed;

    RETURN consumed <= p_limit;
END;
$$;

REVOKE ALL ON FUNCTION meerkateer_consume_rate_limit(text, bytea, integer) FROM PUBLIC;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'meerkateer_app') THEN
        -- init-runtime-role grants default table privileges for tenant data. This global security
        -- table is deliberately exempt: callers only receive the bounded function below.
        REVOKE ALL ON TABLE distributed_rate_limits FROM meerkateer_app;
        GRANT EXECUTE ON FUNCTION meerkateer_consume_rate_limit(text, bytea, integer)
            TO meerkateer_app;
    END IF;
END;
$$;

COMMIT;
