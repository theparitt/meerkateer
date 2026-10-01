-- A monitoring system must expose whether its own durable worker is making
-- progress. This table is installation-scoped (not tenant data) and stores one
-- bounded row per recent worker process. Workers can only write through the
-- SECURITY DEFINER function; the application role can only read the summary.
CREATE TABLE worker_runtime (
    worker_id uuid PRIMARY KEY,
    started_at timestamptz NOT NULL,
    last_cycle_at timestamptz NOT NULL,
    claimed integer NOT NULL CHECK (claimed >= 0),
    completed integer NOT NULL CHECK (completed >= 0),
    retried integer NOT NULL CHECK (retried >= 0),
    dead_lettered integer NOT NULL CHECK (dead_lettered >= 0),
    CHECK (completed + retried + dead_lettered <= claimed)
);

REVOKE ALL ON worker_runtime FROM PUBLIC;

CREATE FUNCTION meerkateer_record_worker_cycle(
    p_worker_id uuid,
    p_started_at timestamptz,
    p_claimed integer,
    p_completed integer,
    p_retried integer,
    p_dead_lettered integer
) RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
BEGIN
    IF p_worker_id IS NULL
       OR p_started_at IS NULL
       OR p_started_at > now() + interval '5 minutes'
       OR p_claimed < 0
       OR p_completed < 0
       OR p_retried < 0
       OR p_dead_lettered < 0
       OR p_completed + p_retried + p_dead_lettered > p_claimed THEN
        RAISE EXCEPTION 'invalid worker cycle';
    END IF;

    INSERT INTO public.worker_runtime (
        worker_id, started_at, last_cycle_at, claimed, completed, retried, dead_lettered
    ) VALUES (
        p_worker_id, p_started_at, now(), p_claimed, p_completed, p_retried, p_dead_lettered
    )
    ON CONFLICT (worker_id) DO UPDATE SET
        last_cycle_at = excluded.last_cycle_at,
        claimed = excluded.claimed,
        completed = excluded.completed,
        retried = excluded.retried,
        dead_lettered = excluded.dead_lettered;

    DELETE FROM public.worker_runtime
    WHERE last_cycle_at < now() - interval '7 days';
END;
$$;

REVOKE ALL ON FUNCTION meerkateer_record_worker_cycle(
    uuid, timestamptz, integer, integer, integer, integer
) FROM PUBLIC;

-- Existing installations already have the runtime roles. Fresh installations
-- repeat these grants in dev/init-runtime-role.sh after creating the roles.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'meerkateer_app') THEN
        GRANT SELECT ON public.worker_runtime TO meerkateer_app;
    END IF;
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'meerkateer_outbox_executor') THEN
        GRANT SELECT, INSERT, UPDATE, DELETE ON public.worker_runtime
            TO meerkateer_outbox_executor;
        ALTER FUNCTION meerkateer_record_worker_cycle(
            uuid, timestamptz, integer, integer, integer, integer
        ) OWNER TO meerkateer_outbox_executor;
    END IF;
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'meerkateer_worker') THEN
        GRANT EXECUTE ON FUNCTION meerkateer_record_worker_cycle(
            uuid, timestamptz, integer, integer, integer, integer
        ) TO meerkateer_worker;
    END IF;
END;
$$;
