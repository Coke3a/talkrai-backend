#!/usr/bin/env bash
set -euo pipefail
: "${TEST_DATABASE_URL:?Set TEST_DATABASE_URL to an EMPTY disposable PostgreSQL database}"
case "$TEST_DATABASE_URL" in
  *localhost*/*test*|*127.0.0.1*/*test*|*@postgres:*/*test*) ;;
  *) echo 'Refusing a database that is not a local/CI test database' >&2; exit 1;;
esac
cd "$(dirname "$0")/.."
cargo test --test web_schema -- --ignored
psql "$TEST_DATABASE_URL" -v ON_ERROR_STOP=1 <<'SQL'
DO $$ BEGIN
 IF to_regclass('public.users') IS NOT NULL THEN RAISE EXCEPTION 'Database must be empty'; END IF;
 IF NOT EXISTS(SELECT 1 FROM pg_roles WHERE rolname='anon') THEN CREATE ROLE anon; END IF;
 IF NOT EXISTS(SELECT 1 FROM pg_roles WHERE rolname='authenticated') THEN CREATE ROLE authenticated; END IF;
 IF NOT EXISTS(SELECT 1 FROM pg_roles WHERE rolname='service_role') THEN CREATE ROLE service_role BYPASSRLS; END IF;
END $$;
CREATE SCHEMA auth;
CREATE FUNCTION auth.uid() RETURNS uuid LANGUAGE sql AS $$ SELECT null::uuid $$;
-- Match Supabase's function defaults; REVOKE FROM PUBLIC alone is insufficient.
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT EXECUTE ON FUNCTIONS TO anon,authenticated,service_role;
SQL
for migration in migrations/*/up.sql; do
  psql "$TEST_DATABASE_URL" -1 -v ON_ERROR_STOP=1 -f "$migration" >/dev/null
done
psql "$TEST_DATABASE_URL" -v ON_ERROR_STOP=1 -f tests/web_transactions.sql
psql "$TEST_DATABASE_URL" -v ON_ERROR_STOP=1 -f tests/web_security.sql
cargo test --test web_http --test web_concurrency --test web_delivery -- --ignored
