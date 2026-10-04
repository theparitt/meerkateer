#!/usr/bin/env sh
set -eu

# Upgrade migrations added after the original Community installation.
# Take a backup before upgrading a persistent installation.
env_file="${MEERKATEER_ENV_FILE:-.env}"
compose_file="${MEERKATEER_COMPOSE_FILE:-compose.yaml}"

compose() {
    docker compose --env-file "$env_file" -f "$compose_file" "$@"
}

query() {
    compose exec -T -e "MEERKATEER_MIGRATION_DB=${MEERKATEER_MIGRATION_DB:-}" postgres sh -c \
        'psql --username "$POSTGRES_USER" --dbname "${MEERKATEER_MIGRATION_DB:-$POSTGRES_DB}" --set=ON_ERROR_STOP=1 -Atc "$1"' \
        sh "$1"
}

apply() {
    compose exec -T -e "MEERKATEER_MIGRATION_DB=${MEERKATEER_MIGRATION_DB:-}" postgres sh -c \
        'psql --username "$POSTGRES_USER" --dbname "${MEERKATEER_MIGRATION_DB:-$POSTGRES_DB}" --set=ON_ERROR_STOP=1' \
        < "$1"
}

workspace_state="$(query "SELECT to_regclass('public.project_agents') IS NOT NULL")"
if [ "$workspace_state" = "f" ]; then
    apply migrations/0008_workspace_agents.sql
fi
password_state="$(query "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = 'public' AND table_name = 'users' AND column_name = 'local_password_hash')")"
if [ "$password_state" = "f" ]; then
    apply migrations/0009_community_owner_password.sql
fi
game_state="$(query "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = 'public' AND table_name = 'services' AND column_name = 'game_kind')")"
if [ "$game_state" = "f" ]; then
    apply migrations/0010_minecraft_instances.sql
fi
operations_state="$(query "SELECT to_regclass('public.incidents') IS NOT NULL")"
if [ "$operations_state" = "f" ]; then
    apply migrations/0011_operations.sql
fi
incident_workflow_state="$(query "SELECT to_regclass('public.incident_activity') IS NOT NULL")"
if [ "$incident_workflow_state" = "f" ]; then
    apply migrations/0012_incident_workflow.sql
fi
alert_replay_state="$(query "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = 'public' AND table_name = 'alert_policies' AND column_name = 'cooldown_seconds')")"
if [ "$alert_replay_state" = "f" ]; then
    apply migrations/0013_alert_replay_cooldown.sql
fi
worker_progress_state="$(query "SELECT to_regclass('public.worker_runtime') IS NOT NULL")"
if [ "$worker_progress_state" = "f" ]; then
    apply migrations/0014_worker_progress.sql
fi
invitation_state="$(query "SELECT to_regclass('public.member_invitations') IS NOT NULL")"
if [ "$invitation_state" = "f" ]; then
    apply migrations/0015_internal_member_invitations.sql
fi
member_recovery_state="$(query "SELECT to_regclass('public.member_password_resets') IS NOT NULL")"
if [ "$member_recovery_state" = "f" ]; then
    apply migrations/0016_member_access_recovery.sql
fi
scheduled_probe_state="$(query "SELECT to_regclass('public.service_probes') IS NOT NULL")"
if [ "$scheduled_probe_state" = "f" ]; then
    apply migrations/0017_scheduled_probes.sql
fi
distributed_rate_limit_state="$(query "SELECT to_regclass('public.distributed_rate_limits') IS NOT NULL")"
if [ "$distributed_rate_limit_state" = "f" ]; then
    apply migrations/0018_distributed_rate_limits.sql
fi
replay_index_unique="$(query "SELECT coalesce((SELECT indisunique FROM pg_index WHERE indexrelid = to_regclass('public.alert_deliveries_replay_idx')), false)")"
if [ "$replay_index_unique" = "f" ]; then
    query "DROP INDEX IF EXISTS alert_deliveries_replay_idx; CREATE UNIQUE INDEX alert_deliveries_replay_idx ON alert_deliveries (tenant_id, replay_of) WHERE replay_of IS NOT NULL" >/dev/null
fi

test "$(query "SELECT to_regclass('public.project_agents') IS NOT NULL")" = "t"
test "$(query "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = 'public' AND table_name = 'users' AND column_name = 'local_password_hash')")" = "t"
test "$(query "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = 'public' AND table_name = 'services' AND column_name = 'game_kind')")" = "t"
test "$(query "SELECT to_regclass('public.incidents') IS NOT NULL")" = "t"
test "$(query "SELECT to_regclass('public.incident_activity') IS NOT NULL")" = "t"
test "$(query "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = 'public' AND table_name = 'alert_policies' AND column_name = 'cooldown_seconds')")" = "t"
test "$(query "SELECT to_regclass('public.worker_runtime') IS NOT NULL")" = "t"
test "$(query "SELECT to_regclass('public.member_invitations') IS NOT NULL")" = "t"
test "$(query "SELECT to_regclass('public.member_password_resets') IS NOT NULL")" = "t"
test "$(query "SELECT to_regclass('public.service_probes') IS NOT NULL")" = "t"
test "$(query "SELECT to_regclass('public.distributed_rate_limits') IS NOT NULL")" = "t"
echo 'Community schema upgraded through migration 0018.'
