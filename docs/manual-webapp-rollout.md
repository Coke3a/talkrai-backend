# Manual Supabase rollout: consolidated migration 023

Production database migrations are operator-managed. `deploy.yml` does not run migrations or start a PostgreSQL service. Database contract tests can be run locally with `scripts/test-web-db.sh` against disposable PostgreSQL; there is no separate web contract test workflow.

## Before applying SQL

1. Take/verify a recoverable Supabase backup. Confirm the database and schema are the intended production target.
2. Stop admitting new LINE generations, let existing jobs and detached summary tasks finish, then stop the old backend workers while applying SQL. `WEB_ADMISSIONS_ENABLED=false` closes web mutations, **not** legacy LINE traffic. The preflight query alone is not a traffic lock.
3. Run `scripts/web-preflight.sql` read-only and resolve duplicate active LINE stories, generation jobs, credit references and payment links. Do not delete records automatically. Review inactive users: legacy `status` represents LINE friendship; account suspension is now separate.
4. Confirm migrations through 022 are present, except the previously observed missing 021. Migration 023 supplies the weekly check-in configuration if absent without overwriting an existing value; it does not claim that migration 021 ran or remove its legacy configuration keys.
5. This consolidated 023 replaces the unpublished 023–028 files. Do not apply it to a database that already ran any earlier split web migration. Local test databases with that schema should be recreated. A production database with split migrations needs a separately reviewed upgrade, not replay.

## Apply in Supabase SQL Editor

Run the **entire** `migrations/00000000000023_web_foundation/up.sql` in one transaction as the database migration owner (normally `postgres`). Wrap the pasted file with `BEGIN;` and `COMMIT;`. Before the final `COMMIT`, record its version for Diesel:

```sql
INSERT INTO public.__diesel_schema_migrations(version)
VALUES ('00000000000023');
```

Keep that insert in the same transaction so an error rolls back schema and history together. When using `diesel migration run` instead, let Diesel manage history; do not insert it manually. Existing migration 021 remains a separate historical migration; review/apply it separately if maintaining a fully sequential Diesel history.

The SQL aborts if legacy generation jobs are still pending/processing. It is forward-only; `down.sql` intentionally refuses destructive rollback.

Verify after commit:

```sql
SELECT public.talkrai_schema_version(); -- 23
SELECT version FROM public.__diesel_schema_migrations ORDER BY version;
SELECT has_function_privilege('anon','public.web_login(jsonb)','EXECUTE'),
       has_function_privilege('authenticated','public.web_login(jsonb)','EXECUTE');
-- Both false. Check all backend-only functions with the same policy.
```

## Deploy and verify

1. Deploy the complete new backend revision after migration succeeds. Startup and `/ready-check` require schema version 23; a missing schema is a hard failure even with `WEB_ENABLED=false`, because legacy repositories also use the new columns.
2. Keep `DEV_AUTH_BYPASS_ENABLED=false` (unset also defaults false). Configure `WEB_ENABLED=true`, `WEB_ORIGIN=https://app.talkrai.app`, `WEB_TERMS_VERSION=2`, Google/LINE Login credentials, and initially `WEB_ADMISSIONS_ENABLED=false`.
3. `WEB_ENABLED=true` also enables shared LINE generation. Do not mix old and new generation workers. Do not roll back to an old binary that assumes every user has a LINE ID.
4. Check `/ready-check` = 200, `/api/public/scenes?limit=1` = 200, and anonymous `/api/web/me` = 401. Validate real LINE/Google redirects and callback cookies on `app.talkrai.app`. New registrations and web story mutations require admissions enabled; enable only in the intended controlled test/launch window.
5. Verify account linking, existing LINE accounts, LINE generation/delivery, web generation and credit settlement, and signed payment callbacks. Local fake-provider tests do not verify provider credentials or console configuration.

## Delivery guarantees and recovery

Each outbox worker claims one delivery for 90 seconds. Its network attempt has a 45-second deadline. Completion is fenced by a unique lease token; a stale worker cannot acknowledge another worker's claim. Push retries reuse the job UUID as the LINE retry key, recognize an already-accepted request, and stop within 23 hours (LINE's deduplication window is 24 hours). A crashed final attempt becomes `failed` after expiry; monitor failed deliveries for reconciliation.

LINE reply and push are separate external operations. An ambiguous reply response or process crash after LINE accepts a reply cannot be made atomic with our database, and fallback to push can still duplicate a reply. No exactly-once external delivery guarantee is claimed. Stored chat content and credit settlement remain transactionally idempotent.

For rollback, close web admissions and use a schema-compatible backend. Retain the forward schema and reconcile interrupted external payments/deliveries; do not replay credit grants manually.

## Verification of this revision

On 2026-09-18: formatting, Clippy across all targets/features, 176 Rust tests (including the explicitly enabled PostgreSQL tests), SQL transaction/security assertions, and the release binary build passed locally. A fresh database passed migrations 001–023. A separate upgrade fixture with 001–020 plus 022 (matching the observed production history) retained its existing LINE identity and credit balance after 023 and received the missing weekly check-in configuration. Production SQL and deployment were not executed.

LINE retry behavior follows https://developers.line.biz/en/docs/messaging-api/retrying-api-request/; real provider callbacks and LINE/Beam acceptance still require production or staging validation.
