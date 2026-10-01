#!/usr/bin/env sh
set -eu

if [ -z "${MEERKATEER_APP_PASSWORD:-}" ]; then
    echo 'MEERKATEER_APP_PASSWORD is required to initialize the runtime role.' >&2
    exit 1
fi
if [ -z "${MEERKATEER_WORKER_PASSWORD:-}" ]; then
    echo 'MEERKATEER_WORKER_PASSWORD is required to initialize the worker role.' >&2
    exit 1
fi

psql --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" \
    --set=ON_ERROR_STOP=1 \
    --set=database_name="$POSTGRES_DB" \
    --set=runtime_password="$MEERKATEER_APP_PASSWORD" \
    --set=worker_password="$MEERKATEER_WORKER_PASSWORD" <<'SQL'
CREATE ROLE meerkateer_app LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION
    PASSWORD :'runtime_password';
CREATE ROLE meerkateer_worker LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION
    PASSWORD :'worker_password';
CREATE ROLE meerkateer_outbox_executor NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE
    NOREPLICATION BYPASSRLS;

GRANT CONNECT ON DATABASE :"database_name" TO meerkateer_app;
GRANT USAGE ON SCHEMA public TO meerkateer_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO meerkateer_app;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO meerkateer_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO meerkateer_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO meerkateer_app;

GRANT CONNECT ON DATABASE :"database_name" TO meerkateer_worker;
GRANT USAGE ON SCHEMA public TO meerkateer_worker, meerkateer_outbox_executor;
GRANT SELECT, UPDATE ON outbox TO meerkateer_outbox_executor;
GRANT SELECT, INSERT ON outbox_dead_letters TO meerkateer_outbox_executor;
GRANT SELECT, UPDATE ON alert_deliveries TO meerkateer_outbox_executor;
GRANT SELECT, INSERT, UPDATE, DELETE ON worker_runtime TO meerkateer_outbox_executor;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO meerkateer_outbox_executor;

ALTER FUNCTION meerkateer_claim_outbox(uuid, integer) OWNER TO meerkateer_outbox_executor;
ALTER FUNCTION meerkateer_complete_outbox(uuid, uuid) OWNER TO meerkateer_outbox_executor;
ALTER FUNCTION meerkateer_retry_outbox(uuid, uuid, text, integer) OWNER TO meerkateer_outbox_executor;
ALTER FUNCTION meerkateer_dead_letter_outbox(uuid, uuid, text) OWNER TO meerkateer_outbox_executor;
ALTER FUNCTION meerkateer_record_worker_cycle(uuid, timestamptz, integer, integer, integer, integer)
    OWNER TO meerkateer_outbox_executor;

GRANT EXECUTE ON FUNCTION meerkateer_claim_outbox(uuid, integer) TO meerkateer_worker;
GRANT EXECUTE ON FUNCTION meerkateer_complete_outbox(uuid, uuid) TO meerkateer_worker;
GRANT EXECUTE ON FUNCTION meerkateer_retry_outbox(uuid, uuid, text, integer) TO meerkateer_worker;
GRANT EXECUTE ON FUNCTION meerkateer_dead_letter_outbox(uuid, uuid, text) TO meerkateer_worker;
GRANT EXECUTE ON FUNCTION meerkateer_record_worker_cycle(
    uuid, timestamptz, integer, integer, integer, integer
) TO meerkateer_worker;
SQL
