#!/usr/bin/env sh
set -eu

# Upgrade migrations added after the original Community installation.
# Take a backup before upgrading a persistent installation.
query() {
    docker compose exec -T -e "MEERKATEER_MIGRATION_DB=${MEERKATEER_MIGRATION_DB:-}" postgres sh -c \
        'psql --username "$POSTGRES_USER" --dbname "${MEERKATEER_MIGRATION_DB:-$POSTGRES_DB}" --set=ON_ERROR_STOP=1 -Atc "$1"' \
        sh "$1"
}

apply() {
    docker compose exec -T -e "MEERKATEER_MIGRATION_DB=${MEERKATEER_MIGRATION_DB:-}" postgres sh -c \
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

test "$(query "SELECT to_regclass('public.project_agents') IS NOT NULL")" = "t"
test "$(query "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = 'public' AND table_name = 'users' AND column_name = 'local_password_hash')")" = "t"
test "$(query "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = 'public' AND table_name = 'services' AND column_name = 'game_kind')")" = "t"
echo 'Community schema upgraded through migration 0010.'
