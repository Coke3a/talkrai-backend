-- Behavioural checks for migration 024 (shared stories). Runs in one transaction and rolls back.
-- Fixed UUIDs: user ...01, character ...c1, scenes ...51/...52, stories A ...a1 (LINE), B ...b1 (web), C ...c2 (LINE).
BEGIN;

INSERT INTO users(id,line_user_id,display_name,terms_accepted_at,account_status,last_check_in_on)
VALUES('00000000-0000-0000-0000-000000000001','Ucheck','Check',now(),'active',(now() AT TIME ZONE 'Asia/Bangkok')::date);
INSERT INTO credit_balances(user_id,balance) VALUES('00000000-0000-0000-0000-000000000001',100);
INSERT INTO app_config(key,value) VALUES('daily_checkin_weekly_credits','1,1,1,1,1,1,1') ON CONFLICT DO NOTHING;
INSERT INTO characters(id,name,personality,speaking_style,background,system_prompt)
VALUES('00000000-0000-0000-0000-0000000000c1','Char','p','s','b','sp');
INSERT INTO scenes(id,character_id,name,location,time_of_day,atmosphere,situation_prompt,opening_narrator,opening_dialogue)
VALUES('00000000-0000-0000-0000-000000000051','00000000-0000-0000-0000-0000000000c1','S1','l','day','a','sp','n','d'),
      ('00000000-0000-0000-0000-000000000052','00000000-0000-0000-0000-0000000000c1','S2','l','day','a','sp','n','d');
INSERT INTO roleplay_sessions(id,user_id,character_id,scene_id,interaction_channel,status) VALUES
('00000000-0000-0000-0000-0000000000a1','00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-0000000000c1','00000000-0000-0000-0000-000000000051','line','active'),
('00000000-0000-0000-0000-0000000000b1','00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-0000000000c1','00000000-0000-0000-0000-000000000052','web','active');

-- 1. A web turn on story B is admitted.
DO $$ BEGIN
 ASSERT admit_turn('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-0000000000b1','web','turn','k1','{"content":"hi","expected_version":0}')->>'status'='pending', '1: web turn on B should be pending';
END $$;

-- 2. Another story of the same user runs concurrently; two turns reserve 4 credits.
DO $$ BEGIN
 ASSERT admit_turn('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-0000000000a1','web','turn','k2','{"content":"hi","expected_version":0}')->>'status'='pending', '2: web turn on A should be pending while B is busy';
 ASSERT (SELECT reserved FROM credit_balances WHERE user_id='00000000-0000-0000-0000-000000000001')=4, '2: reserved should be 4';
END $$;

-- 3. The same story busy on the other surface vs the same surface; neither charges.
DO $$ BEGIN
 ASSERT admit_turn('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-0000000000a1','line','turn','line:e1','{"content":"x","reply_token":"t"}')->>'error'='TURN_IN_PROGRESS_ELSEWHERE', '3: LINE send during a web turn should be TURN_IN_PROGRESS_ELSEWHERE';
 ASSERT admit_turn('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-0000000000a1','web','turn','k3','{"content":"hi","expected_version":0}')->>'error'='TURN_IN_PROGRESS', '3: second web send on A should be TURN_IN_PROGRESS';
 ASSERT (SELECT reserved FROM credit_balances WHERE user_id='00000000-0000-0000-0000-000000000001')=4, '3: rejected admits must not reserve credits';
END $$;

-- 4. Regeneration is retired.
DO $$ BEGIN
 ASSERT admit_turn('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-0000000000a1','web','regeneration','k4','{"message_id":"00000000-0000-0000-0000-0000000000f0"}')->>'error'='VALIDATION_ERROR', '4: regeneration should be VALIDATION_ERROR';
END $$;

-- 5. After A's web turn settles, LINE can send without a version; a stale web version is rejected.
UPDATE jobs SET status='completed' WHERE request_key='k2';
UPDATE roleplay_sessions SET context_version=1 WHERE id='00000000-0000-0000-0000-0000000000a1';
DO $$ BEGIN
 ASSERT admit_turn('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-0000000000a1','line','turn','line:e2','{"content":"x","reply_token":"t"}')->>'status'='pending', '5: LINE should send to a story the web just used';
END $$;
UPDATE jobs SET status='completed' WHERE request_key='line:e2';
UPDATE roleplay_sessions SET context_version=2 WHERE id='00000000-0000-0000-0000-0000000000a1';
DO $$ BEGIN
 ASSERT admit_turn('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-0000000000a1','web','turn','k5','{"content":"hi","expected_version":1}')->>'error'='STALE_VERSION', '5: web send with an old version should be STALE_VERSION';
END $$;

-- 6. web_story scopes pending_job to its own story and exposes only can_send.
DO $$ BEGIN
 ASSERT web_story('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-0000000000a1')->'pending_job' = 'null'::jsonb, '6: A has no pending job even though B does';
 ASSERT web_story('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-0000000000b1')->'pending_job'->>'origin'='web', '6: B pending_job should carry origin web';
 ASSERT web_story('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-0000000000a1')->'capabilities' = '{"can_send": true}'::jsonb, '6: capabilities should be exactly can_send';
END $$;

-- 7. Transfer is retired.
DO $$ BEGIN
 ASSERT web_mutate_story('00000000-0000-0000-0000-000000000001','transfer','{"session_id":"00000000-0000-0000-0000-0000000000a1","expected_version":2}','key-t')->>'error'='NOT_FOUND', '7: transfer should be NOT_FOUND';
END $$;

-- 8. Creating a story is not blocked by another story's pending turn.
DO $$ DECLARE r jsonb; BEGIN
 r:=web_mutate_story('00000000-0000-0000-0000-000000000001','create','{"scene_id":"00000000-0000-0000-0000-000000000052","persona":{"name":"n","description":"d"}}','key-c');
 ASSERT r->>'error' IS NULL AND r->>'id' IS NOT NULL, '8: create should succeed while B has a pending job, got '||r::text;
 ASSERT r->>'interaction_channel'='web', '8: created story should be a web story';
END $$;

-- 9. Resuming an ended LINE story from the web must not touch LINE's current selection.
UPDATE roleplay_sessions SET status='ended' WHERE id='00000000-0000-0000-0000-0000000000a1';
INSERT INTO roleplay_sessions(id,user_id,character_id,scene_id,interaction_channel,status) VALUES
('00000000-0000-0000-0000-0000000000c2','00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-0000000000c1','00000000-0000-0000-0000-000000000051','line','active');
DO $$ DECLARE r jsonb; BEGIN
 r:=web_mutate_story('00000000-0000-0000-0000-000000000001','resume',jsonb_build_object('session_id','00000000-0000-0000-0000-0000000000a1','expected_version',(SELECT context_version FROM roleplay_sessions WHERE id='00000000-0000-0000-0000-0000000000a1')),'');
 ASSERT r->>'error' IS NULL, '9: resume of ended LINE story should succeed, got '||r::text;
 ASSERT (SELECT status||'/'||interaction_channel FROM roleplay_sessions WHERE id='00000000-0000-0000-0000-0000000000a1')='active/web', '9: resumed story should be active/web';
 ASSERT (SELECT status||'/'||interaction_channel FROM roleplay_sessions WHERE id='00000000-0000-0000-0000-0000000000c2')='active/line', '9: LINE selection must be unchanged';
 r:=web_mutate_story('00000000-0000-0000-0000-000000000001','resume',jsonb_build_object('session_id','00000000-0000-0000-0000-0000000000c2','expected_version',(SELECT context_version FROM roleplay_sessions WHERE id='00000000-0000-0000-0000-0000000000c2')),'');
 ASSERT r->>'error' IS NULL, '9: resume of active LINE story should succeed, got '||r::text;
 ASSERT (SELECT interaction_channel FROM roleplay_sessions WHERE id='00000000-0000-0000-0000-0000000000c2')='line', '9: active LINE story keeps channel line on resume';
END $$;

-- 10. Persona edits work on LINE stories.
DO $$ DECLARE r jsonb; BEGIN
 r:=web_mutate_story('00000000-0000-0000-0000-000000000001','persona',jsonb_build_object('session_id','00000000-0000-0000-0000-0000000000c2','expected_version',(SELECT context_version FROM roleplay_sessions WHERE id='00000000-0000-0000-0000-0000000000c2'),'persona','{"name":"Mint","description":"x"}'::jsonb),'');
 ASSERT r->>'error' IS NULL AND r->'persona'->>'name'='Mint', '10: persona edit on a LINE story should succeed, got '||r::text;
END $$;

-- 11. Schema version.
DO $$ BEGIN ASSERT talkrai_schema_version()=24, '11: schema version should be 24'; END $$;

ROLLBACK;
