# Shared Stories (LINE + Web) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One story can be played on both LINE and the webapp without "transfer"; the webapp can chat with several characters at once; "regenerate" is removed from the product.

**Architecture:** Behaviour lives in Postgres functions (`admit_turn`, `web_story`, `web_mutate_story`) from migration 023. Migration 024 redefines them with `CREATE OR REPLACE` (no data change, no table change, reversible). The turn lock moves from per-user to per-story. `roleplay_sessions.interaction_channel='line'` keeps its LINE meaning ("the story LINE has selected", enforced by the existing unique index `one_active_line_story`), but no longer restricts which surface may send. LINE replies carry a one-line notice when web turns happened since the last LINE turn; the webapp shows a notice when LINE turns arrived.

**Tech Stack:** Rust (Axum, diesel-async, raw SQL), PostgreSQL plpgsql, Next.js 16 / React 19 / SWR (webapp, separate repo `/Users/coke/Projects/talkrai/webapp`).

**Spec:** Agreed with the user in conversation (2026-10-05). Summary, verbatim decisions:
1. Web: chat with any number of characters **concurrently** (send to B while A is still replying).
2. LINE: unchanged — one selected story at a time; changing requires picking a character in LIFF.
3. A story started in LINE is visible and **sendable** on web without transfer; message order is shared (LINE msg1 → web msg2 → LINE msg3).
4. Selecting/creating stories on web must NOT change what LINE has selected. Web‑created stories are NOT reachable from LINE (this release).
5. LINE: first reply after web activity carries `มีการคุยต่อบนเว็บ N ข้อความ ดูได้ที่ <link>`.
6. Web: when LINE turns arrived, show `มีการคุยต่อใน LINE N ข้อความ`; history reloads; the typed draft is never lost.
7. If the character is mid‑reply to the other surface: LINE says it is answering on web, send again; web says it is answering from LINE, send again.
8. Remove "ตอบใหม่" (regenerate) completely. Remove transfer-to-web.
9. **Do not deploy, do not push to `main`, do not run migrations against production.** Work on local branch `feat/shared-stories` in both repos. Production has real users.

## Global Constraints

- Branch `feat/shared-stories` in `/Users/coke/Projects/talkrai/backend` and `/Users/coke/Projects/talkrai/webapp`. Never `git push`. Never `fly deploy`, `wrangler deploy`, `diesel migration run` against a remote DB, or Supabase MCP writes.
- `schema.rs` is hand-written; this plan changes no tables, so do not touch it and do not run `diesel print-schema`.
- Backend CI gates: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.
- Webapp gates: `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm format:check`, `pnpm build`.
- Keep the code style of the file you edit (dense one-line SQL in Rust strings, prettier formatting in webapp).
- User-facing copy is Thai, exactly as written in this plan.
- Turn cost stays 2 credits. Rate limit stays 30 jobs/min/user.
- Keep `settle_turn` and `usecases/roleplay/generate.rs` regeneration handling unchanged: regeneration jobs already admitted before rollout must still drain. (Follow-up release may delete them.)

## Review Focus

1. Two admits for the **same story** from different surfaces at once → exactly one admitted; the other gets `TURN_IN_PROGRESS_ELSEWHERE` (no double charge). Pinned in Task 1 SQL check.
2. Web send with an old `expected_version` after a LINE turn → `STALE_VERSION`, web reloads history, draft text stays in the composer. Pinned in Task 1 (SQL) + Task 5 (helper test).
3. Web "resume" of a LINE story that LINE already ended must not hijack LINE's selection / violate `one_active_line_story` when LINE has another active story. Pinned in Task 1.
4. LINE notice count must not count LINE's own turns, failed web turns, or web turns before the previous LINE message. Pinned in Task 3 SQL check + unit test.
5. Old clients during rollout (old webapp calling `/regenerations` or `/transfer-to-web`; old backend calling `admit_turn(...,'regeneration',...)`) get a clean 4xx, not a 500. Pinned in Task 1 and Task 2.

---

### Task 1: Migration 024 (SQL functions) + local Postgres verification

**Files:**
- Create: `backend/migrations/00000000000024_shared_stories/up.sql`
- Create: `backend/migrations/00000000000024_shared_stories/down.sql`
- Create: `backend/tests/sql/024_shared_stories_check.sql`
- Create: `backend/scripts/verify-migrations.sh`

**Interfaces:**
- Produces: `admit_turn` error codes `TURN_IN_PROGRESS` (same surface busy on this story), `TURN_IN_PROGRESS_ELSEWHERE` (other surface busy on this story), `VALIDATION_ERROR` (kind ≠ `turn`). `web_story(...)` JSON: `pending_job` scoped to this story with new key `origin`; `capabilities` = `{ "can_send": bool }` only. `web_mutate_story` accepts only `create|resume|persona`, others → `{"error":"NOT_FOUND"}`. `talkrai_schema_version()` = 24.

- [ ] **Step 1: Create branch**

```bash
cd /Users/coke/Projects/talkrai/backend && git checkout -b feat/shared-stories
```

- [ ] **Step 2: Write `up.sql`** exactly:

```sql
-- Shared stories: every story can be continued on the web; LINE still keeps one selected story
-- (interaction_channel='line' + status='active', unique per user). Turns lock per story, not per user.
-- Regeneration and LINE-to-web transfer are retired. Functions only; no data or table changes.
CREATE OR REPLACE FUNCTION web_story(p_user uuid,p_id uuid) RETURNS jsonb LANGUAGE sql STABLE AS $$
 SELECT jsonb_build_object('id',s.id,'status',s.status,'interaction_channel',s.interaction_channel,'context_version',s.context_version,'persona',s.persona,'mood',s.mood,'relationship_level',s.relationship_level,'updated_at',s.updated_at,
 'scene',jsonb_build_object('id',c.id,'name',c.name,'image_url',c.image_url,'available',c.is_active AND a.is_active),
 'character',jsonb_build_object('id',a.id,'name',a.name,'avatar_url',a.avatar_url),
 'pending_job',(SELECT jsonb_build_object('id',j.id,'status',j.status,'session_id',j.session_id,'kind',j.kind,'origin',j.origin,'result',j.result,'error',null) FROM jobs j WHERE j.session_id=s.id AND j.user_id=p_user AND j.mode='roleplay_message' AND j.status IN('pending','processing') LIMIT 1),
 'capabilities',jsonb_build_object('can_send',s.status='active' AND c.is_active AND a.is_active))
 FROM roleplay_sessions s JOIN scenes c ON c.id=s.scene_id JOIN characters a ON a.id=s.character_id WHERE s.user_id=p_user AND s.id=p_id
$$;

CREATE OR REPLACE FUNCTION web_mutate_story(p_user uuid,p_operation text,p jsonb,p_key text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE s roleplay_sessions; sc scenes; existing request_deduplications; new_id uuid;
BEGIN
 IF p_operation NOT IN ('create','resume','persona') THEN RETURN jsonb_build_object('error','NOT_FOUND'); END IF;
 PERFORM 1 FROM users WHERE id=p_user AND account_status='active' FOR UPDATE;
 IF NOT FOUND THEN RETURN jsonb_build_object('error','ACCOUNT_SUSPENDED'); END IF;
 IF p_operation='create' THEN
  SELECT * INTO existing FROM request_deduplications WHERE user_id=p_user AND operation=p_operation AND request_key=p_key;
  IF FOUND THEN
   IF existing.payload<>p THEN RETURN jsonb_build_object('error','IDEMPOTENCY_MISMATCH'); END IF;
   RETURN web_story(p_user,existing.result_id);
  END IF;
  PERFORM 1 FROM users WHERE id=p_user AND terms_accepted_at IS NOT NULL;
  IF NOT FOUND THEN RETURN jsonb_build_object('error','TERMS_REQUIRED'); END IF;
  SELECT * INTO sc FROM scenes WHERE id=(p->>'scene_id')::uuid AND is_active;
  IF NOT FOUND OR NOT EXISTS(SELECT 1 FROM characters WHERE id=sc.character_id AND is_active) THEN RETURN jsonb_build_object('error','SCENE_UNAVAILABLE'); END IF;
  INSERT INTO roleplay_sessions(user_id,character_id,scene_id,mood,relationship_level,interaction_channel,persona) VALUES(p_user,sc.character_id,sc.id,sc.start_mood,sc.start_relationship_level,'web',p->'persona') RETURNING id INTO new_id;
  INSERT INTO analytics_events(user_id,event_name,properties,client_event_id,occurred_at) VALUES(p_user,'session_created',jsonb_build_object('surface','web','session_id',new_id),gen_random_uuid(),now());
  INSERT INTO messages(session_id,role,content) VALUES(new_id,'character','*'||sc.opening_narrator||'*'||E'\n'||sc.opening_dialogue);
  INSERT INTO request_deduplications(user_id,operation,request_key,payload,result_id) VALUES(p_user,p_operation,p_key,p,new_id);
 ELSE
  SELECT * INTO s FROM roleplay_sessions WHERE id=(p->>'session_id')::uuid AND user_id=p_user FOR UPDATE;
  IF NOT FOUND THEN RETURN jsonb_build_object('error','NOT_FOUND'); END IF;
  IF s.context_version<>(p->>'expected_version')::bigint THEN RETURN jsonb_build_object('error','STALE_VERSION'); END IF;
  IF NOT EXISTS(SELECT 1 FROM scenes available_scene JOIN characters c ON available_scene.character_id=c.id WHERE available_scene.id=s.scene_id AND available_scene.is_active AND c.is_active) THEN RETURN jsonb_build_object('error','SCENE_UNAVAILABLE'); END IF;
  IF EXISTS(SELECT 1 FROM jobs WHERE session_id=s.id AND mode='roleplay_message' AND status IN ('pending','processing')) THEN RETURN jsonb_build_object('error','TURN_IN_PROGRESS'); END IF;
  -- Resuming a story LINE has ended makes it a web story, so LINE's own selection is never changed from the web.
  UPDATE roleplay_sessions SET status=CASE WHEN p_operation='resume' THEN 'active' ELSE status END,interaction_channel=CASE WHEN p_operation='resume' AND s.status<>'active' THEN 'web' ELSE interaction_channel END,persona=CASE WHEN p_operation='persona' THEN p->'persona' ELSE persona END,context_version=context_version+1,updated_at=now() WHERE id=s.id;
  new_id:=s.id;
 END IF;
 RETURN web_story(p_user,new_id);
END $$;

CREATE OR REPLACE FUNCTION admit_turn(p_user uuid,p_session uuid,p_origin text,p_kind text,p_key text,p jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE s roleplay_sessions; u users; old jobs; busy_origin text; j uuid; today date:=(now() AT TIME ZONE 'Asia/Bangkok')::date; streak integer; amount integer; curve integer[]; added integer;
BEGIN
 IF p_kind<>'turn' THEN RETURN jsonb_build_object('error','VALIDATION_ERROR'); END IF;
 SELECT * INTO u FROM users WHERE id=p_user FOR UPDATE;
 IF NOT FOUND OR u.account_status<>'active' THEN RETURN jsonb_build_object('error','ACCOUNT_SUSPENDED'); END IF;
 SELECT * INTO old FROM jobs WHERE user_id=p_user AND request_key=p_key;
 IF FOUND THEN
  IF (old.request_payload-'reply_token')<>(p-'reply_token') OR old.session_id<>p_session OR old.kind<>p_kind OR old.origin<>p_origin THEN RETURN jsonb_build_object('error','IDEMPOTENCY_MISMATCH'); END IF;
  RETURN jsonb_build_object('id',old.id,'status',old.status,'kind',old.kind,'session_id',old.session_id,'result',old.result,'error',old.failed_reason);
 END IF;
 IF (SELECT count(*) FROM jobs WHERE user_id=p_user AND created_at>now()-interval '1 minute')>=30 THEN RETURN jsonb_build_object('error','RATE_LIMITED'); END IF;
 SELECT * INTO s FROM roleplay_sessions WHERE id=p_session AND user_id=p_user FOR UPDATE;
 IF NOT FOUND THEN RETURN jsonb_build_object('error','NOT_FOUND'); END IF;
 IF s.status<>'active' OR NOT EXISTS(SELECT 1 FROM scenes sc JOIN characters c ON c.id=sc.character_id WHERE sc.id=s.scene_id AND sc.is_active AND c.is_active) THEN RETURN jsonb_build_object('error','SCENE_UNAVAILABLE'); END IF;
 IF u.terms_accepted_at IS NULL THEN RETURN jsonb_build_object('error','TERMS_REQUIRED'); END IF;
 -- One reply at a time per story; other stories of the same user run concurrently.
 SELECT origin INTO busy_origin FROM jobs WHERE session_id=p_session AND mode='roleplay_message' AND status IN('pending','processing') LIMIT 1;
 IF FOUND THEN RETURN jsonb_build_object('error',CASE WHEN busy_origin=p_origin THEN 'TURN_IN_PROGRESS' ELSE 'TURN_IN_PROGRESS_ELSEWHERE' END); END IF;
 IF p_origin='web' AND s.context_version<>(p->>'expected_version')::bigint THEN RETURN jsonb_build_object('error','STALE_VERSION'); END IF;
 IF length(btrim(p->>'content')) NOT BETWEEN 1 AND 4000 THEN RETURN jsonb_build_object('error','VALIDATION_ERROR'); END IF;
 IF u.last_check_in_on IS DISTINCT FROM today THEN
  streak:=CASE WHEN u.last_check_in_on=today-1 THEN u.check_in_streak+1 ELSE 1 END;
  SELECT string_to_array(value,',')::integer[] INTO curve FROM app_config WHERE key='daily_checkin_weekly_credits';
  IF array_length(curve,1)<>7 OR curve IS NULL THEN RAISE EXCEPTION 'Invalid daily grant configuration'; END IF;
  amount:=curve[((streak-1)%7)+1];
  INSERT INTO credit_grants(user_id,grant_kind,period_key,amount) VALUES(p_user,'daily',today::text,amount) ON CONFLICT DO NOTHING;
  GET DIAGNOSTICS added=ROW_COUNT;
  IF added=1 THEN
   UPDATE users SET check_in_streak=streak,longest_streak=greatest(longest_streak,streak),last_check_in_on=today WHERE id=p_user;
   UPDATE credit_balances SET balance=balance+amount,updated_at=now() WHERE user_id=p_user;
   IF amount>0 THEN INSERT INTO credit_transactions(user_id,type,amount,balance_after,description) SELECT p_user,'bonus',amount,balance,'Daily check-in' FROM credit_balances WHERE user_id=p_user; END IF;
  END IF;
 END IF;
 UPDATE credit_balances SET reserved=reserved+2 WHERE user_id=p_user AND balance-reserved>=2;
 IF NOT FOUND THEN RETURN jsonb_build_object('error','INSUFFICIENT_CREDITS'); END IF;
 INSERT INTO jobs(session_id,user_id,line_user_id,user_message,origin,kind,request_key,request_payload,context_version,checkpoint,target_message_id,reservation,reply_token)
 VALUES(p_session,p_user,CASE WHEN p_origin='line' THEN u.line_user_id END,coalesce(p->>'content',''),p_origin,p_kind,p_key,p,s.context_version,to_jsonb(s),NULL,2,p->>'reply_token') RETURNING id INTO j;
 RETURN jsonb_build_object('id',j,'status','pending','kind',p_kind,'session_id',p_session,'result',null,'error',null);
END $$;

CREATE OR REPLACE FUNCTION talkrai_schema_version() RETURNS integer LANGUAGE sql STABLE AS $$ SELECT 24 $$;
```

Notes for the implementer (do not deviate):
- `CREATE OR REPLACE` keeps the existing REVOKE/GRANT on these functions; do not re-grant.
- The busy check is deliberately BEFORE the `STALE_VERSION` check, so a web send during a LINE reply says "answering from LINE" instead of "stale".
- Before writing, diff this text against the 023 originals (`sed -n 140,184p` and `189,234p` of `migrations/00000000000023_web_foundation/up.sql`) and confirm the ONLY behavioural differences are those listed in the Interfaces block plus: create is no longer blocked by other stories' jobs; persona/resume work on LINE stories; no `CHANNEL_MISMATCH`. If the 023 text differs in anything else (column names etc.), keep the 023 text for that part.

- [ ] **Step 3: Write `down.sql`**: copy verbatim from `migrations/00000000000023_web_foundation/up.sql` the full `CREATE FUNCTION web_story ...`, `web_mutate_story ...`, `admit_turn ...` definitions, changing `CREATE FUNCTION` → `CREATE OR REPLACE FUNCTION`, then `CREATE OR REPLACE FUNCTION talkrai_schema_version() RETURNS integer LANGUAGE sql STABLE AS $$ SELECT 23 $$;`. Header comment: `-- Restores the 023 function bodies. Safe: no data changes.`

- [ ] **Step 4: Write `scripts/verify-migrations.sh`** — spins a throwaway local Postgres in Docker (image `postgres:18-alpine`, already pulled), applies every `migrations/*/up.sql` in order with `psql -v ON_ERROR_STOP=1`, runs the check file, then runs 024 `down.sql` + `up.sql` again and re-runs the check, then removes the container. It must refuse to run if `DATABASE_URL` points anywhere but its own container (do not read `.env`; set `PGHOST=127.0.0.1` and a random port explicitly).

```bash
#!/usr/bin/env bash
# Applies all migrations to a throwaway local Postgres and runs SQL checks. Never touches a real database.
set -euo pipefail
cd "$(dirname "$0")/.."
name="talkrai-migcheck-$$"; port=$((20000 + RANDOM % 10000))
docker run -d --rm --name "$name" -e POSTGRES_PASSWORD=check -p "127.0.0.1:$port:5432" postgres:18-alpine >/dev/null
trap 'docker rm -f "$name" >/dev/null 2>&1 || true' EXIT
export PGHOST=127.0.0.1 PGPORT=$port PGUSER=postgres PGPASSWORD=check PGDATABASE=postgres
until pg_isready -q; do sleep 1; done
run() { psql -q -v ON_ERROR_STOP=1 -f "$1"; }
for dir in migrations/*/; do [ -f "$dir/up.sql" ] && run "$dir/up.sql"; done
run tests/sql/024_shared_stories_check.sql
run migrations/00000000000024_shared_stories/down.sql
[ "$(psql -tAc 'SELECT talkrai_schema_version()')" = 23 ]
run migrations/00000000000024_shared_stories/up.sql
run tests/sql/024_shared_stories_check.sql
echo "migrations + shared stories checks: OK"
```

If an older migration needs Supabase-only objects (roles/schemas such as `service_role`, `auth`), create the minimum stubs in a `tests/sql/00_supabase_stubs.sql` run before the loop, and note it in the report. Do not edit old migrations. Use `/opt/homebrew/opt/libpq/bin` for `psql`/`pg_isready` (prepend to PATH inside the script if not found).

- [ ] **Step 5: Write `tests/sql/024_shared_stories_check.sql`** — a single transaction (`BEGIN; ... ROLLBACK;`) that seeds fixtures and asserts with `DO $$ ... ASSERT ... $$`. Seed: one user (terms accepted, `account_status='active'`, `last_check_in_on` = today in Asia/Bangkok so no daily grant), `credit_balances` balance 100 reserved 0, `app_config` row `daily_checkin_weekly_credits='1,1,1,1,1,1,1'` if absent, one active character with two active scenes, story A (`interaction_channel='line'`, `status='active'`), story B (`interaction_channel='web'`, `status='active'`). Read real column names from migrations; fill NOT NULL columns with simple values. Assertions (each a separate `DO` block with a message):
  1. `admit_turn(u,B,'web','turn','k1',{"content":"hi","expected_version":0})->>'status'='pending'`.
  2. While B is pending: `admit_turn(u,A,'web','turn','k2',{content, expected_version:0})` is pending (concurrent across stories). Reserved credits now 4.
  3. While A has a web job pending: `admit_turn(u,A,'line','turn','line:e1',{"content":"x","reply_token":"t"})->>'error'='TURN_IN_PROGRESS_ELSEWHERE'`; `admit_turn(u,A,'web','turn','k3',...)->>'error'='TURN_IN_PROGRESS'`; reserved still 4 (no charge).
  4. `admit_turn(u,A,'web','regeneration','k4','{"message_id":"..."}')->>'error'='VALIDATION_ERROR'`.
  5. Mark A's web job `completed` and bump A `context_version` to 1 (direct UPDATE, simulating settle); then `admit_turn(u,A,'line',...)` is pending (LINE can send to a story the web just used; no version needed); then after marking it completed + version 2, `admit_turn(u,A,'web',..., expected_version 1)->>'error'='STALE_VERSION'`.
  6. `web_story(u,A)->'pending_job'` is null after completion; while a job on B is pending, `web_story(u,A)->'pending_job'` IS null and `web_story(u,B)->'pending_job'->>'origin'='web'`; `web_story(u,A)->'capabilities'` = `{"can_send": true}` exactly.
  7. `web_mutate_story(u,'transfer',{session_id:A,expected_version:..},'key-t')->>'error'='NOT_FOUND'`.
  8. `web_mutate_story(u,'create',{scene_id, persona},'key-c')` succeeds while B has a pending job (create not blocked).
  9. Resume rules: set A `status='ended'` (LINE ended it) and create story C `interaction_channel='line', status='active'` (LINE's new selection). `web_mutate_story(u,'resume',{session_id:A, expected_version:<A's>},'')` → A status `active`, channel `web`; C unchanged and still `line`/`active` (unique index not violated). Resume on an already-active `line` story keeps channel `line`.
  10. `web_mutate_story(u,'persona',...)` on a `line` story succeeds (no CHANNEL_MISMATCH).
  11. `talkrai_schema_version()=24`.

- [ ] **Step 6: Run** `bash scripts/verify-migrations.sh` → expect final line `migrations + shared stories checks: OK`. Fix SQL (not the assertions) until green. If an assertion itself is wrong against the spec, say so in the report.

- [ ] **Step 7: Commit**

```bash
git add migrations/00000000000024_shared_stories scripts/verify-migrations.sh tests/sql
git commit -m "feat(db): share stories across LINE and web, lock turns per story, retire regeneration"
```

---

### Task 2: Backend Rust — remove regenerate/transfer, new error codes, schema 24

**Files:**
- Modify: `backend/src/handlers/routers/web/routes.rs` (routes at ~155,158; macros at ~523,548)
- Modify: `backend/src/usecases/web/stories.rs`
- Modify: `backend/src/infra/db/repositories/web_data_postgres.rs` (error map ~25-50; `WebRead::Messages` ~59; `WebRead::Credits` ~60; `WebRead::Job` ~62)
- Modify: `backend/src/usecases/webhook/receive_webhook.rs` (~255-265)
- Modify: `backend/src/infra/db/schema_check.rs`
- Modify: `backend/docs/web-openapi.json`

**Interfaces:**
- Consumes: Task 1 error codes.
- Produces (HTTP JSON the webapp relies on): `GET /api/web/sessions/{id}` → `capabilities: {can_send}`, `pending_job.origin: "line"|"web"`. `GET /api/web/sessions/{id}/messages` items have `origin: "line"|"web"|null` and no `is_regeneratable`. `GET /api/web/jobs/{id}` has `origin`. `GET /api/web/credits` has no `cost_per_regeneration`. Error `TURN_IN_PROGRESS_ELSEWHERE` → HTTP 409 (falls in the existing `_ => 409` arm). Routes `/regenerations` and `/transfer-to-web` → 404 (removed).

- [ ] **Step 1: Write failing tests** in `src/usecases/web/stories.rs` `#[cfg(test)] mod tests` (create if absent) using a stub `TurnRepository` that records calls:
  - `turn(owner, session, "regeneration", "valid-key-123456", json!({"expected_version":0,"message_id":Uuid::new_v4()}))` → `Err(WebError::Rejected("VALIDATION_ERROR"))` and the stub was never called.
  - `mutate(owner, "transfer", <StoryMutation with session_id + expected_version>, key)` → `Err(WebError::Rejected("VALIDATION_ERROR"))` without calling the data repo.
  Check how `validate_key` defines a valid key and use one. If existing test helpers/mocks for these traits exist, reuse them.
- [ ] **Step 2: Run** `cargo test --lib usecases::web` → FAIL.
- [ ] **Step 3: Implement**
  - `stories.rs`: in `mutate`, first line: `if !matches!(operation, "create" | "resume" | "persona") { return Err(WebError::Rejected("VALIDATION_ERROR")); }`; change `matches!(operation, "create" | "transfer")` → `operation == "create"`. In `turn`, replace the `else if input["message_id"]...` branch: before content validation add `if kind != "turn" { return Err(WebError::Rejected("VALIDATION_ERROR")); }` and drop the message_id branch.
  - `routes.rs`: delete the `transfer-to-web` and `regenerations` routes and the `mutate!(transfer, "transfer");` and `generation!(regenerate, "regeneration");` lines.
  - `web_data_postgres.rs`: error map add `"TURN_IN_PROGRESS_ELSEWHERE" => "TURN_IN_PROGRESS_ELSEWHERE",` and `"VALIDATION_ERROR" => "VALIDATION_ERROR",`; remove `"CHANNEL_MISMATCH"` (no longer produced). Keep `INVALID_REGENERATION` (draining settle path). Messages query: replace `'is_regeneratable',m.role='character' AND m.turn_id IS NOT NULL AND m.sequence=(SELECT max(sequence) FROM messages WHERE session_id=m.session_id)` with `'origin',(SELECT origin FROM jobs WHERE id=m.turn_id)`. Credits query: remove `'cost_per_regeneration',2,`. Job query: add `'origin',origin,` after `'kind',kind,`.
  - If `turn_postgres.rs` uses its own error mapping (check ~line 36 onward), add the same two codes there too.
  - `receive_webhook.rs`: add arm before the catch-all:
    ```rust
    crate::domain::web::WebError::Rejected("TURN_IN_PROGRESS_ELSEWHERE") => {
        "ตัวละครกำลังตอบข้อความจากเว็บอยู่ ส่งใหม่อีกครั้งได้เลยเมื่อตอบเสร็จ"
    }
    ```
  - `schema_check.rs`: `row.version == 24`, message `"Apply migration 024 before deployment"`.
  - `docs/web-openapi.json`: remove the two paths and the removed fields; add `origin` where added; add `TURN_IN_PROGRESS_ELSEWHERE` wherever error codes are enumerated. Keep JSON valid (`node -e 'JSON.parse(require("fs").readFileSync("docs/web-openapi.json"))'`).
  - If any compile error appears because a function/import became unused, remove only that orphan.
- [ ] **Step 4: Run** `cargo fmt && cargo clippy -- -D warnings && cargo test` → all PASS.
- [ ] **Step 5: Commit** `git commit -am "feat(web): drop regenerate and transfer, report cross-surface busy, require schema 24"`

---

### Task 3: LINE reply notice for web activity

**Files:**
- Modify: `backend/src/infra/line/shared_delivery.rs`
- Modify: `backend/src/handlers/routers/web/runtime.rs` (~105-120, the `deliver_pending(...)` call; `WEB_ORIGIN` is read at ~40 into `origin`)
- Modify: `backend/tests/sql/024_shared_stories_check.sql` (add count assertion)

**Interfaces:**
- Consumes: nothing new from Tasks 1-2.
- Produces: `pub fn web_activity_notice(web_turns: i64, web_origin: &str, session_id: Uuid) -> Option<String>`; `deliver_pending(pool, line, web_origin: String)`.

- [ ] **Step 1: Failing unit tests** in `shared_delivery.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_notice_without_web_turns() {
        assert_eq!(web_activity_notice(0, "https://app.talkrai.app", Uuid::nil()), None);
    }
    #[test]
    fn notice_counts_web_turns_and_links_story() {
        let id = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
        assert_eq!(
            web_activity_notice(3, "https://app.talkrai.app/", id).as_deref(),
            Some("มีการคุยต่อบนเว็บ 3 ข้อความ ดูได้ที่ https://app.talkrai.app/stories/11111111-2222-3333-4444-555555555555")
        );
    }
}
```

- [ ] **Step 2: Run** `cargo test --lib infra::line::shared_delivery` → FAIL.
- [ ] **Step 3: Implement**
  - `web_activity_notice`: `(web_turns > 0).then(|| format!("มีการคุยต่อบนเว็บ {web_turns} ข้อความ ดูได้ที่ {}/stories/{session_id}", web_origin.trim_end_matches('/')))`.
  - `Delivery` struct: add `#[diesel(sql_type=SqlUuid)] session_id: Uuid,` and `#[diesel(sql_type=diesel::sql_types::BigInt)] web_turns: i64,`.
  - Selection CTE: remove `AND s.interaction_channel='line'` (and the now-unneeded `JOIN roleplay_sessions s` in `selected` only if nothing else there uses `s`). Rationale: `origin='line'` already means the reply belongs in LINE; transfer no longer exists.
  - Final SELECT: add `,j.session_id,(SELECT count(*) FROM jobs w WHERE w.session_id=j.session_id AND w.origin='web' AND w.kind='turn' AND w.status='completed' AND w.completed_at<j.created_at AND w.completed_at>coalesce((SELECT max(p.created_at) FROM jobs p WHERE p.session_id=j.session_id AND p.origin='line' AND p.id<>j.id AND p.created_at<j.created_at),'-infinity'::timestamptz)) AS web_turns`.
  - Signature `deliver_pending(pool: Arc<PgPool>, line: Arc<dyn LineClient>, web_origin: String)`. Build `let mut messages = Vec::new(); if let Some(text) = web_activity_notice(item.web_turns, &web_origin, item.session_id) { messages.push(LineMessage::Text { text, sender_name: String::new(), sender_icon_url: String::new() }); } messages.push(message);` and pass `messages` (clone for reply attempt) to both `reply_messages` and `push_messages_with_retry_key`. Notice goes BEFORE the character reply.
  - Failed generation path (content falls back to the apology text) gets the notice too — acceptable; do not special-case.
  - `runtime.rs`: pass the runtime's `origin` string (clone it before the `tokio::spawn`) as the third argument.
- [ ] **Step 4: SQL check** — append to `tests/sql/024_shared_stories_check.sql` a `DO` block before `ROLLBACK` that builds on a fresh story D: LINE job L1 (completed, created t0), web turns W1, W2 completed after t0, one web job failed after t0, LINE job L2 created after W2. Assert the `web_turns` subquery (copy it, with `j` = L2) returns 2; and for L1 returns 0.
- [ ] **Step 5: Run** `cargo fmt && cargo clippy -- -D warnings && cargo test && bash scripts/verify-migrations.sh` → all PASS.
- [ ] **Step 6: Commit** `git commit -am "feat(line): tell LINE players how many turns happened on the web"`

---

### Task 4: Webapp — remove regenerate & transfer, let LINE stories be played on web

Repo: `/Users/coke/Projects/talkrai/webapp`. First: `git checkout -b feat/shared-stories`.

**Files:**
- Modify: `app/lib/types.ts` (Job, Story.capabilities, Message, Credits)
- Modify: `app/components/chat.tsx`
- Modify: `app/components/message-history.tsx`
- Modify: `app/components/credits.tsx:127`
- Modify: `app/lib/optimistic.ts` (delete `isStaleRegeneration`)
- Modify: `app/lib/proxy.ts:13`
- Modify: `app/(member)/stories/page.tsx:22-31`
- Modify: `tests/optimistic.test.ts`, `tests/mock-backend.mjs`

**Interfaces:**
- Consumes: Task 2 JSON (`capabilities.can_send` only; `Message.origin`; `Job.origin`; no `cost_per_regeneration`).
- Produces: `Message.origin: "line" | "web" | null`, `Job.origin?: "line" | "web"` for Task 5.

- [ ] **Step 1: Update tests first**: in `tests/optimistic.test.ts` delete every `isStaleRegeneration` test and import, and the regeneration case inside the `isStalePending`/`pendingTurnContent` tests only if they import removed symbols (keep tests of remaining functions). In `tests/mock-backend.mjs`: remove `is_regeneratable`, `can_regenerate`, `can_transfer_to_web`, `cost_per_regeneration`, the `/regenerations` handler; add `origin: null` to messages it builds (`"web"` for turn messages) and `origin: "web"` to jobs.
- [ ] **Step 2: Types** — `Job`: add `origin?: "line" | "web";`. `Story.capabilities`: `{ can_send: boolean }`. `Message`: replace `is_regeneratable: boolean` with `origin: "line" | "web" | null`. `Credits`: remove `cost_per_regeneration`.
- [ ] **Step 3: Remove regenerate** — `message-history.tsx`: remove `onRegenerate`, `disabled`, `cost` props from `CharacterMessage` and `MessageHistory` and the regenerate button (keep `CopyButton`); drop `Refresh` import if unused. `chat.tsx`: remove `regenerate`, `requestRegenerate`, `isStaleRegeneration` import, and the props passed to `MessageHistory`. `credits.tsx`: delete the `ขอคำตอบใหม่` `<li>`. `optimistic.ts`: delete `isStaleRegeneration` and its comment.
- [ ] **Step 4: Remove transfer and the confirmation dialog** — `chat.tsx`: delete `transfer()`, `Confirmation` type, `confirmation` state, `confirm()`, `cancelConfirm`, and the whole confirm `<StageDialog>`; delete the `story.interaction_channel === "line"` notice branch in the dock so LINE stories fall through to the `status`/terms/Composer branches. `PersonaForm`: delete the `interaction_channel === "line"` muted paragraph, and in the caller change `disabled={busy || mutating || story.interaction_channel === "line"}` → `disabled={busy || mutating}`. Remove imports that became unused (`ChatIcon`, `Refresh` if unused, etc. — let `pnpm lint`/`tsc` tell you). `proxy.ts`: regex `(resume|turns)`.
- [ ] **Step 5: Stories list chip** — `stories/page.tsx`: LINE story chip text `เล่นใน LINE` → `เล่นได้ทั้ง LINE และเว็บ` (keep the dot). Web chip unchanged.
- [ ] **Step 6: Run** `pnpm lint && pnpm typecheck && pnpm test && pnpm format:check && pnpm build` → PASS. `grep -rnE "regenerat|transfer|is_regeneratable|can_transfer" app tests` → no hits except none.
- [ ] **Step 7: Commit** `git commit -am "feat: play LINE stories on the web, remove regenerate and transfer"`

---

### Task 5: Webapp — cross-surface awareness (LINE activity notice, busy-from-LINE, stale reload)

**Files:**
- Create: `app/lib/surface.ts`
- Create: `tests/surface.test.ts`
- Modify: `app/lib/api.ts` (error messages map)
- Modify: `app/components/chat.tsx`

**Interfaces:**
- Consumes: `Message.origin`, `Job.origin` (Task 4).
- Produces: `countLineTurns(messages: Pick<Message,"role"|"origin"|"sequence">[], after: number): number`.

- [ ] **Step 1: Failing test** `tests/surface.test.ts`:

```ts
import test from "node:test";
import assert from "node:assert/strict";
import { countLineTurns } from "../app/lib/surface.ts";

const m = (sequence: number, role: "user" | "character", origin: "line" | "web" | null) => ({ sequence, role, origin });

test("counts LINE player turns after the baseline only", () => {
  const messages = [m(1, "character", null), m(2, "user", "line"), m(3, "character", "line"), m(4, "user", "web"), m(5, "character", "web"), m(6, "user", "line"), m(7, "character", "line")];
  assert.equal(countLineTurns(messages, 1), 2);
  assert.equal(countLineTurns(messages, 5), 1);
  assert.equal(countLineTurns(messages, 7), 0);
});

test("web turns and opening lines are never counted", () => {
  assert.equal(countLineTurns([m(1, "character", null), m(2, "user", "web")], 0), 0);
});
```

(Match the import style of existing tests in `tests/*.test.ts` — check one first.)
- [ ] **Step 2: Run** `pnpm test` → FAIL (module missing).
- [ ] **Step 3: Implement** `app/lib/surface.ts`:

```ts
import type { Message } from "./types";

// Player turns sent from LINE after the given sequence; each LINE turn stores one user message.
export function countLineTurns(
  messages: Pick<Message, "role" | "origin" | "sequence">[],
  after: number,
) {
  return messages.filter(
    (message) =>
      message.role === "user" &&
      message.origin === "line" &&
      message.sequence > after,
  ).length;
}
```

- [ ] **Step 4: api.ts messages** — add `TURN_IN_PROGRESS_ELSEWHERE: "ตัวละครกำลังตอบข้อความจาก LINE อยู่ ข้อความของคุณยังอยู่ ส่งอีกครั้งได้เมื่อตอบเสร็จ",` and change `STALE_VERSION` to `"มีการคุยต่อใน LINE เรื่องอัปเดตแล้ว ข้อความของคุณยังอยู่ ส่งอีกครั้งได้เลย"`. Change `TURN_IN_PROGRESS` to `"ตัวละครกำลังตอบข้อความก่อนหน้าอยู่ รอสักครู่แล้วส่งอีกครั้ง"`.
- [ ] **Step 5: chat.tsx wiring**
  - `handleError`: when `error instanceof ApiError && error.status === 409`, also `void refreshMessages();` (add to deps). The existing `send` catch already restores the typed text to the composer — keep it.
  - LINE activity notice (derived, no effect): `const [lineSeen, setLineSeen] = useState(() => lastSequence(initialMessages.items));` then `const lineTurns = countLineTurns(allMessages, lineSeen);`. In the dock, before the `error` notice, render when `lineTurns > 0`:
    ```tsx
    <div className="stage-notice" role="status">
      <ChatIcon size={18} />
      <div>
        <p>มีการคุยต่อใน LINE {lineTurns} ข้อความ</p>
        <button type="button" className="stage-notice-action subtle" onClick={() => setLineSeen(lastSequence(allMessages))}>
          รับทราบ
        </button>
      </div>
    </div>
    ```
    Use the existing `lastSequence` from `@/app/lib/optimistic` (check its signature accepts `Message[]`; if it returns something else, compute `Math.max(0, ...messages.map(m => m.sequence))` inline instead).
  - Typing indicator: replace the `activeJob && activeJob.session_id !== id ? "กำลังสร้างคำตอบในอีกเรื่องหนึ่ง…" : ...` ternary with `activeJob?.origin === "line" ? \`${story.character.name} กำลังตอบข้อความจาก LINE…\` : \`${story.character.name} กำลังเขียน…\``.
  - Job-finished effect: the `"สร้างคำตอบไม่สำเร็จ ไม่ได้หักเครดิต ..."` error must only show when the finished job was started on this page: guard with `job.origin !== "line"` (a failed LINE reply is LINE's to report).
  - Keep `ChatIcon` import (re-used here) — Task 4 may have removed it; re-add if so.
- [ ] **Step 6: Run** `pnpm lint && pnpm typecheck && pnpm test && pnpm format:check && pnpm build` → PASS.
- [ ] **Step 7: Commit** `git add -A && git commit -m "feat(chat): show LINE activity and LINE replies in progress"`

---

## Rollout (for the user — NOT executed by agents)

Order matters; the backend refuses to start unless schema version = 24, and the running old backend keeps working on the new functions (they are backward compatible except that regenerate/transfer now return 4xx).

1. Review branches `feat/shared-stories` (backend, webapp). Run `bash scripts/verify-migrations.sh` locally.
2. Check no `roleplay_message` job is pending/processing in prod (optional; functions swap atomically).
3. Apply `migrations/00000000000024_shared_stories/up.sql` to prod (one transaction) and insert `00000000000024` into `__diesel_schema_migrations` (see memory: prod migration 021 gap — do not run a blanket `diesel migration run`).
4. Immediately merge + push backend `main` (CI deploys to Fly). Until it is live, a backend restart would fail the schema check — keep the window short.
5. Merge + push webapp `main` (Cloudflare Workers Builds deploys).
6. Smoke: LINE send → reply; open the same story on web, send → reply; LINE send → reply starts with "มีการคุยต่อบนเว็บ 1 ข้อความ …"; web shows "มีการคุยต่อใน LINE 1 ข้อความ".
7. Rollback: apply `down.sql` (restores 023 functions, version 23) and redeploy previous backend + webapp commits.
