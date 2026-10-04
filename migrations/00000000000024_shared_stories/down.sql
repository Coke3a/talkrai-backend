-- Restores the 023 function bodies and the one-pending-job-per-user index. No data changes;
-- the index recreate fails if a user currently has pending jobs on two stories (drain them first).
DROP INDEX IF EXISTS one_generation_per_story;
CREATE UNIQUE INDEX IF NOT EXISTS one_generation_per_user ON jobs(user_id) WHERE mode='roleplay_message' AND status IN ('pending','processing');
CREATE OR REPLACE FUNCTION web_story(p_user uuid,p_id uuid) RETURNS jsonb LANGUAGE sql STABLE AS $$
 SELECT jsonb_build_object('id',s.id,'status',s.status,'interaction_channel',s.interaction_channel,'context_version',s.context_version,'persona',s.persona,'mood',s.mood,'relationship_level',s.relationship_level,'updated_at',s.updated_at,
 'scene',jsonb_build_object('id',c.id,'name',c.name,'image_url',c.image_url,'available',c.is_active AND a.is_active),
 'character',jsonb_build_object('id',a.id,'name',a.name,'avatar_url',a.avatar_url),
 'pending_job',(SELECT jsonb_build_object('id',j.id,'status',j.status,'session_id',j.session_id,'kind',j.kind,'result',j.result,'error',null) FROM jobs j WHERE j.user_id=p_user AND j.mode='roleplay_message' AND j.status IN('pending','processing') LIMIT 1),
 'capabilities',jsonb_build_object('can_send',s.interaction_channel='web' AND s.status='active' AND c.is_active AND a.is_active,'can_transfer_to_web',s.interaction_channel='line' AND c.is_active AND a.is_active,'can_regenerate',s.interaction_channel='web' AND s.status='active' AND c.is_active AND a.is_active AND EXISTS(SELECT 1 FROM messages m WHERE m.session_id=s.id AND m.role='character' AND m.turn_id IS NOT NULL)))
 FROM roleplay_sessions s JOIN scenes c ON c.id=s.scene_id JOIN characters a ON a.id=s.character_id WHERE s.user_id=p_user AND s.id=p_id
$$;
CREATE OR REPLACE FUNCTION web_mutate_story(p_user uuid,p_operation text,p jsonb,p_key text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE s roleplay_sessions; sc scenes; existing request_deduplications; new_id uuid; opening text;
BEGIN
 PERFORM 1 FROM users WHERE id=p_user AND account_status='active' FOR UPDATE;
 IF NOT FOUND THEN RETURN jsonb_build_object('error','ACCOUNT_SUSPENDED'); END IF;
 IF p_operation IN ('create','transfer') THEN
  SELECT * INTO existing FROM request_deduplications WHERE user_id=p_user AND operation=p_operation AND request_key=p_key;
  IF FOUND THEN
   IF existing.payload<>p THEN RETURN jsonb_build_object('error','IDEMPOTENCY_MISMATCH'); END IF;
   RETURN web_story(p_user,existing.result_id);
  END IF;
 END IF;
 IF EXISTS(SELECT 1 FROM jobs WHERE user_id=p_user AND mode='roleplay_message' AND status IN ('pending','processing')) THEN RETURN jsonb_build_object('error','TURN_IN_PROGRESS'); END IF;
 IF p_operation='create' THEN
  PERFORM 1 FROM users WHERE id=p_user AND terms_accepted_at IS NOT NULL;
  IF NOT FOUND THEN RETURN jsonb_build_object('error','TERMS_REQUIRED'); END IF;
  SELECT * INTO sc FROM scenes WHERE id=(p->>'scene_id')::uuid AND is_active;
  IF NOT FOUND OR NOT EXISTS(SELECT 1 FROM characters WHERE id=sc.character_id AND is_active) THEN RETURN jsonb_build_object('error','SCENE_UNAVAILABLE'); END IF;
  INSERT INTO roleplay_sessions(user_id,character_id,scene_id,mood,relationship_level,interaction_channel,persona) VALUES(p_user,sc.character_id,sc.id,sc.start_mood,sc.start_relationship_level,'web',p->'persona') RETURNING id INTO new_id;
  INSERT INTO analytics_events(user_id,event_name,properties,client_event_id,occurred_at) VALUES(p_user,'session_created',jsonb_build_object('surface','web','session_id',new_id),gen_random_uuid(),now());
  INSERT INTO messages(session_id,role,content) VALUES(new_id,'character','*'||sc.opening_narrator||'*'||E'\n'||sc.opening_dialogue);
 ELSE
  SELECT * INTO s FROM roleplay_sessions WHERE id=(p->>'session_id')::uuid AND user_id=p_user FOR UPDATE;
  IF NOT FOUND THEN RETURN jsonb_build_object('error','NOT_FOUND'); END IF;
  IF s.context_version<>(p->>'expected_version')::bigint THEN RETURN jsonb_build_object('error','STALE_VERSION'); END IF;
  IF NOT EXISTS(SELECT 1 FROM scenes available_scene JOIN characters c ON available_scene.character_id=c.id WHERE available_scene.id=s.scene_id AND available_scene.is_active AND c.is_active) THEN RETURN jsonb_build_object('error','SCENE_UNAVAILABLE'); END IF;
  IF (p_operation='transfer' AND s.interaction_channel<>'line') OR (p_operation<>'transfer' AND s.interaction_channel<>'web') THEN RETURN jsonb_build_object('error','CHANNEL_MISMATCH'); END IF;
  UPDATE roleplay_sessions SET interaction_channel='web',status=CASE WHEN p_operation='persona' THEN status ELSE 'active' END,persona=CASE WHEN p_operation='persona' THEN p->'persona' ELSE persona END,context_version=context_version+1,updated_at=now() WHERE id=s.id;
  new_id:=s.id;
 END IF;
 IF p_operation IN ('create','transfer') THEN INSERT INTO request_deduplications(user_id,operation,request_key,payload,result_id) VALUES(p_user,p_operation,p_key,p,new_id); END IF;
 RETURN web_story(p_user,new_id);
END $$;
CREATE OR REPLACE FUNCTION admit_turn(p_user uuid,p_session uuid,p_origin text,p_kind text,p_key text,p jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE s roleplay_sessions; u users; old jobs; target messages; j uuid; today date:=(now() AT TIME ZONE 'Asia/Bangkok')::date; streak integer; amount integer; curve integer[]; added integer; checkpoint jsonb;
BEGIN
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
 IF s.interaction_channel<>p_origin THEN RETURN jsonb_build_object('error','CHANNEL_MISMATCH'); END IF;
 IF s.status<>'active' OR NOT EXISTS(SELECT 1 FROM scenes sc JOIN characters c ON c.id=sc.character_id WHERE sc.id=s.scene_id AND sc.is_active AND c.is_active) THEN RETURN jsonb_build_object('error','SCENE_UNAVAILABLE'); END IF;
 IF u.terms_accepted_at IS NULL THEN RETURN jsonb_build_object('error','TERMS_REQUIRED'); END IF;
 IF p_origin='web' AND s.context_version<>(p->>'expected_version')::bigint THEN RETURN jsonb_build_object('error','STALE_VERSION'); END IF;
 IF EXISTS(SELECT 1 FROM jobs WHERE user_id=p_user AND mode='roleplay_message' AND status IN('pending','processing')) THEN RETURN jsonb_build_object('error','TURN_IN_PROGRESS'); END IF;
 checkpoint:=to_jsonb(s);
 IF p_kind='regeneration' THEN
  SELECT * INTO target FROM messages WHERE session_id=p_session ORDER BY sequence DESC LIMIT 1;
  IF target.id IS DISTINCT FROM (p->>'message_id')::uuid OR target.role<>'character' OR target.turn_id IS NULL THEN RETURN jsonb_build_object('error','INVALID_REGENERATION'); END IF;
  SELECT jobs.checkpoint INTO checkpoint FROM jobs WHERE id=target.turn_id;
  IF checkpoint IS NULL THEN RETURN jsonb_build_object('error','INVALID_REGENERATION'); END IF;
  checkpoint:=checkpoint||jsonb_build_object('persona',s.persona);
 ELSE
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
 END IF;
 UPDATE credit_balances SET reserved=reserved+2 WHERE user_id=p_user AND balance-reserved>=2;
 IF NOT FOUND THEN RETURN jsonb_build_object('error','INSUFFICIENT_CREDITS'); END IF;
 INSERT INTO jobs(session_id,user_id,line_user_id,user_message,origin,kind,request_key,request_payload,context_version,checkpoint,target_message_id,reservation,reply_token)
 VALUES(p_session,p_user,CASE WHEN p_origin='line' THEN u.line_user_id END,coalesce(p->>'content',''),p_origin,p_kind,p_key,p,s.context_version,checkpoint,target.id,2,p->>'reply_token') RETURNING id INTO j;
 RETURN jsonb_build_object('id',j,'status','pending','kind',p_kind,'session_id',p_session,'result',null,'error',null);
END $$;

CREATE OR REPLACE FUNCTION talkrai_schema_version() RETURNS integer LANGUAGE sql STABLE AS $$ SELECT 23 $$;
