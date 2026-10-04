#!/usr/bin/env sh
set -eu

project="${MEERKATEER_INTEGRATION_PROJECT:-meerkateer-integration}"
database='meerkateer_upgrade_test'

docker compose -p "$project" exec -T postgres sh -c \
    'psql -U "$POSTGRES_USER" -d postgres -v ON_ERROR_STOP=1 -c "CREATE DATABASE meerkateer_upgrade_test"' >/dev/null
for migration in migrations/000[1-7]_*.sql; do
    docker compose -p "$project" exec -T postgres sh -c \
        'psql -U "$POSTGRES_USER" -d meerkateer_upgrade_test -v ON_ERROR_STOP=1' \
        < "$migration" >/dev/null
done
docker compose -p "$project" exec -T postgres sh -c \
    'psql -U "$POSTGRES_USER" -d meerkateer_upgrade_test -v ON_ERROR_STOP=1' \
    < dev/seed.sql >/dev/null

COMPOSE_PROJECT_NAME="$project" MEERKATEER_MIGRATION_DB="$database" ./scripts/migrate-existing.sh
COMPOSE_PROJECT_NAME="$project" MEERKATEER_MIGRATION_DB="$database" ./scripts/migrate-existing.sh

preserved="$(docker compose -p "$project" exec -T postgres sh -c \
    'psql -U "$POSTGRES_USER" -d meerkateer_upgrade_test -Atc "SELECT count(*) FROM services WHERE slug = '\''fixture-server'\''"' | tr -d '\r')"
test "$preserved" = 1
echo 'Upgrade from 0007 to 0018 preserved seeded service and is repeatable.'
