-- Durable, lease-based outbox delivery. The login worker never receives direct table
-- access: it can only call these narrow SECURITY DEFINER entry points. The
-- meerkateer_outbox_executor role is created and made the function owner by the
-- deployment role initializer after migrations have run.
ALTER TABLE outbox
    ADD COLUMN locked_at timestamptz,
    ADD COLUMN locked_by uuid,
    ADD COLUMN last_error text CHECK (last_error IS NULL OR char_length(last_error) <= 2048),
    ADD COLUMN dead_lettered_at timestamptz,
    ADD CONSTRAINT outbox_terminal_state CHECK (
        dead_lettered_at IS NULL OR processed_at IS NOT NULL
    ),
    ADD CONSTRAINT outbox_lease_pair CHECK (
        (locked_at IS NULL) = (locked_by IS NULL)
    );

CREATE INDEX outbox_claim_idx
ON outbox (available_at, created_at, id)
WHERE processed_at IS NULL;

CREATE TABLE outbox_dead_letters (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    outbox_id uuid NOT NULL UNIQUE REFERENCES outbox(id) ON DELETE RESTRICT,
    topic text NOT NULL CHECK (char_length(topic) BETWEEN 1 AND 128),
    payload jsonb NOT NULL CHECK (jsonb_typeof(payload) = 'object'),
    attempts integer NOT NULL CHECK (attempts > 0),
    last_error text NOT NULL CHECK (char_length(last_error) BETWEEN 1 AND 2048),
    failed_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX outbox_dead_letters_timeline_idx
ON outbox_dead_letters (tenant_id, failed_at DESC, id);

ALTER TABLE outbox_dead_letters ENABLE ROW LEVEL SECURITY;
ALTER TABLE outbox_dead_letters FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON outbox_dead_letters
    USING (tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid);

CREATE FUNCTION meerkateer_claim_outbox(p_worker_id uuid, p_batch_size integer)
RETURNS TABLE (
    id uuid,
    tenant_id uuid,
    topic text,
    payload jsonb,
    attempts integer
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
BEGIN
    IF p_batch_size NOT BETWEEN 1 AND 100 THEN
        RAISE EXCEPTION 'outbox batch size must be between 1 and 100'
            USING ERRCODE = '22023';
    END IF;

    RETURN QUERY
    WITH candidates AS (
        SELECT candidate.id
        FROM public.outbox AS candidate
        WHERE candidate.processed_at IS NULL
          AND candidate.available_at <= clock_timestamp()
          AND (
              candidate.locked_at IS NULL
              OR candidate.locked_at < clock_timestamp() - interval '5 minutes'
          )
        ORDER BY candidate.available_at, candidate.created_at, candidate.id
        FOR UPDATE SKIP LOCKED
        LIMIT p_batch_size
    )
    UPDATE public.outbox AS claimed
    SET locked_at = clock_timestamp(),
        locked_by = p_worker_id,
        attempts = claimed.attempts + 1,
        last_error = NULL
    FROM candidates
    WHERE claimed.id = candidates.id
    RETURNING claimed.id, claimed.tenant_id, claimed.topic, claimed.payload, claimed.attempts;
END;
$$;

CREATE FUNCTION meerkateer_complete_outbox(p_worker_id uuid, p_outbox_id uuid)
RETURNS boolean
LANGUAGE sql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
    WITH completed AS (
        UPDATE public.outbox
        SET processed_at = clock_timestamp(),
            locked_at = NULL,
            locked_by = NULL,
            last_error = NULL
        WHERE id = p_outbox_id
          AND locked_by = p_worker_id
          AND processed_at IS NULL
        RETURNING id
    )
    SELECT EXISTS (SELECT 1 FROM completed);
$$;

CREATE FUNCTION meerkateer_retry_outbox(
    p_worker_id uuid,
    p_outbox_id uuid,
    p_error text,
    p_delay_seconds integer
)
RETURNS boolean
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
DECLARE
    changed boolean;
BEGIN
    IF p_delay_seconds NOT BETWEEN 1 AND 3600 THEN
        RAISE EXCEPTION 'outbox retry delay must be between 1 and 3600 seconds'
            USING ERRCODE = '22023';
    END IF;
    IF p_error IS NULL OR btrim(p_error) = '' THEN
        RAISE EXCEPTION 'outbox retry error must not be empty'
            USING ERRCODE = '22023';
    END IF;

    WITH retried AS (
        UPDATE public.outbox
        SET available_at = clock_timestamp() + make_interval(secs => p_delay_seconds),
            locked_at = NULL,
            locked_by = NULL,
            last_error = left(p_error, 2048)
        WHERE id = p_outbox_id
          AND locked_by = p_worker_id
          AND processed_at IS NULL
        RETURNING id
    )
    SELECT EXISTS (SELECT 1 FROM retried) INTO changed;
    RETURN changed;
END;
$$;

CREATE FUNCTION meerkateer_dead_letter_outbox(
    p_worker_id uuid,
    p_outbox_id uuid,
    p_error text
)
RETURNS boolean
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
DECLARE
    inserted boolean;
BEGIN
    IF p_error IS NULL OR btrim(p_error) = '' THEN
        RAISE EXCEPTION 'dead-letter error must not be empty'
            USING ERRCODE = '22023';
    END IF;

    WITH terminal AS (
        UPDATE public.outbox
        SET processed_at = clock_timestamp(),
            dead_lettered_at = clock_timestamp(),
            locked_at = NULL,
            locked_by = NULL,
            last_error = left(p_error, 2048)
        WHERE id = p_outbox_id
          AND locked_by = p_worker_id
          AND processed_at IS NULL
        RETURNING id, tenant_id, topic, payload, attempts, last_error, dead_lettered_at
    ), recorded AS (
        INSERT INTO public.outbox_dead_letters (
            tenant_id, outbox_id, topic, payload, attempts, last_error, failed_at
        )
        SELECT tenant_id, id, topic, payload, attempts, last_error, dead_lettered_at
        FROM terminal
        ON CONFLICT (outbox_id) DO NOTHING
        RETURNING id
    )
    SELECT EXISTS (SELECT 1 FROM recorded) INTO inserted;
    RETURN inserted;
END;
$$;

REVOKE ALL ON FUNCTION meerkateer_claim_outbox(uuid, integer) FROM PUBLIC;
REVOKE ALL ON FUNCTION meerkateer_complete_outbox(uuid, uuid) FROM PUBLIC;
REVOKE ALL ON FUNCTION meerkateer_retry_outbox(uuid, uuid, text, integer) FROM PUBLIC;
REVOKE ALL ON FUNCTION meerkateer_dead_letter_outbox(uuid, uuid, text) FROM PUBLIC;
