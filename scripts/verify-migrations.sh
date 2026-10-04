#!/usr/bin/env bash
# Applies all migrations to a throwaway local Postgres and runs SQL checks. Never touches a real database.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/opt/libpq/bin:$PATH"
name="talkrai-migcheck-$$"; port=$((20000 + RANDOM % 10000))
docker run -d --rm --name "$name" -e POSTGRES_PASSWORD=check -p "127.0.0.1:$port:5432" postgres:18-alpine >/dev/null
trap 'docker rm -f "$name" >/dev/null 2>&1 || true' EXIT
unset DATABASE_URL
export PGHOST=127.0.0.1 PGPORT=$port PGUSER=postgres PGPASSWORD=check PGDATABASE=postgres
until pg_isready -q; do sleep 1; done
run() { psql -q -v ON_ERROR_STOP=1 -f "$1"; }
run tests/sql/00_supabase_stubs.sql
for dir in migrations/*/; do [ -f "$dir/up.sql" ] && run "$dir/up.sql"; done
run tests/sql/024_shared_stories_check.sql
run migrations/00000000000024_shared_stories/down.sql
[ "$(psql -tAc 'SELECT talkrai_schema_version()')" = 23 ]
run migrations/00000000000024_shared_stories/up.sql
run tests/sql/024_shared_stories_check.sql
run tests/web_security.sql
run tests/web_transactions.sql
echo "migrations + shared stories checks: OK"
