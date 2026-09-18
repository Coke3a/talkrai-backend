-- Apply this entire file in one transaction after draining legacy LINE jobs.
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM jobs WHERE mode='roleplay_message' AND status IN ('pending','processing')) THEN
  RAISE EXCEPTION 'Drain legacy roleplay jobs before applying migration 023';
 END IF;
END $$;
-- Expand only. Audit inactive users and drain legacy generation jobs before enabling web.
ALTER TABLE users ALTER COLUMN line_user_id DROP NOT NULL;
ALTER TABLE users ADD COLUMN account_status varchar NOT NULL DEFAULT 'active' CHECK (account_status IN ('active','suspended'));
ALTER TABLE users ADD COLUMN terms_version text;
ALTER TABLE users ADD COLUMN first_surface text NOT NULL DEFAULT 'line' CHECK(first_surface IN ('line','web'));
ALTER TABLE users ADD COLUMN acquisition_source text NOT NULL DEFAULT 'line';
-- Existing status remains LINE friendship. An unfollow must never suspend the account.
ALTER TABLE roleplay_sessions ADD COLUMN interaction_channel varchar NOT NULL DEFAULT 'line' CHECK (interaction_channel IN ('line','web'));
ALTER TABLE roleplay_sessions ADD COLUMN context_version bigint NOT NULL DEFAULT 0;
ALTER TABLE roleplay_sessions ADD COLUMN persona jsonb NOT NULL DEFAULT '{"name":"","description":""}';
CREATE UNIQUE INDEX one_active_line_story ON roleplay_sessions(user_id) WHERE status='active' AND interaction_channel='line';
CREATE INDEX user_story_activity ON roleplay_sessions(user_id,updated_at DESC,id DESC);
ALTER TABLE credit_balances ADD COLUMN reserved integer NOT NULL DEFAULT 0 CHECK (reserved >= 0 AND reserved <= balance);
CREATE TABLE auth_identities (
 provider text NOT NULL CHECK(provider IN ('line','google')), issuer text NOT NULL, subject text NOT NULL,
 user_id uuid NOT NULL REFERENCES users(id), created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY(provider,issuer,subject)
);
CREATE UNIQUE INDEX auth_identities_owner_provider ON auth_identities(user_id,provider);
-- LINE subjects are provider-scoped. Configure the browser login channel under the existing provider.
INSERT INTO auth_identities(provider,issuer,subject,user_id) SELECT 'line','https://access.line.me',line_user_id,id FROM users WHERE line_user_id IS NOT NULL;
CREATE TABLE web_auth_sessions (
 token_hash text PRIMARY KEY, user_id uuid NOT NULL REFERENCES users(id), csrf_token text NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(), expires_at timestamptz NOT NULL, last_seen_at timestamptz NOT NULL DEFAULT now(), revoked_at timestamptz
);
CREATE INDEX web_auth_sessions_owner ON web_auth_sessions(user_id);
CREATE TABLE auth_flows (
 state_hash text PRIMARY KEY, browser_hash text NOT NULL, provider text NOT NULL, nonce text NOT NULL,
 acquisition_source text NOT NULL DEFAULT 'direct',
 pkce_verifier text NOT NULL, return_to text NOT NULL, link_user_id uuid REFERENCES users(id),
 link_session_hash text REFERENCES web_auth_sessions(token_hash), expires_at timestamptz NOT NULL
);
CREATE INDEX auth_flows_expiry ON auth_flows(expires_at);
CREATE TABLE credit_grants (
 user_id uuid NOT NULL REFERENCES users(id), grant_kind text NOT NULL, period_key text NOT NULL,
 amount integer NOT NULL CHECK(amount >= 0), created_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY(user_id,grant_kind,period_key)
);
-- Preserve prior entitlements: existing balances already received welcome credit.
INSERT INTO credit_grants(user_id,grant_kind,period_key,amount) SELECT user_id,'welcome','once',0 FROM credit_balances;
INSERT INTO credit_grants(user_id,grant_kind,period_key,amount) SELECT id,'daily',last_check_in_on::text,0 FROM users WHERE last_check_in_on IS NOT NULL;
CREATE TABLE request_deduplications (
 user_id uuid NOT NULL REFERENCES users(id), operation text NOT NULL, request_key text NOT NULL, payload jsonb NOT NULL,
 result_id uuid NOT NULL, created_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY(user_id,operation,request_key)
);
ALTER TABLE jobs ALTER COLUMN line_user_id DROP NOT NULL;
ALTER TABLE jobs ADD COLUMN origin varchar NOT NULL DEFAULT 'line' CHECK(origin IN ('line','web'));
ALTER TABLE jobs ADD COLUMN kind varchar NOT NULL DEFAULT 'turn' CHECK(kind IN ('turn','regeneration'));
ALTER TABLE jobs ADD COLUMN request_key text;
ALTER TABLE jobs ADD COLUMN request_payload jsonb;
ALTER TABLE jobs ADD COLUMN context_version bigint;
ALTER TABLE jobs ADD COLUMN checkpoint jsonb;
ALTER TABLE jobs ADD COLUMN input_snapshot jsonb;
ALTER TABLE jobs ADD COLUMN target_message_id uuid REFERENCES messages(id);
ALTER TABLE jobs ADD COLUMN result jsonb;
ALTER TABLE jobs ADD COLUMN reservation integer NOT NULL DEFAULT 0 CHECK(reservation >= 0);
ALTER TABLE jobs ADD COLUMN lease_token uuid;
ALTER TABLE jobs ADD COLUMN lease_expires_at timestamptz;
ALTER TABLE jobs ADD COLUMN delivery_status text NOT NULL DEFAULT 'none' CHECK(delivery_status IN ('none','pending','delivered','failed'));
CREATE UNIQUE INDEX one_generation_per_user ON jobs(user_id) WHERE mode='roleplay_message' AND status IN ('pending','processing');
CREATE UNIQUE INDEX job_request_dedup ON jobs(user_id,request_key) WHERE request_key IS NOT NULL;
CREATE UNIQUE INDEX one_debit_per_job ON credit_transactions(reference_id) WHERE type='consumption' AND reference_id IS NOT NULL;
ALTER TABLE messages ADD COLUMN turn_id uuid REFERENCES jobs(id);
ALTER TABLE messages ADD COLUMN sequence bigint GENERATED BY DEFAULT AS IDENTITY;
-- Deterministic legacy ordering, preserving content and IDs.
WITH ordered AS (SELECT id,row_number() OVER(ORDER BY created_at,id) AS n FROM messages) UPDATE messages m SET sequence=o.n FROM ordered o WHERE m.id=o.id;
SELECT setval(pg_get_serial_sequence('messages','sequence'), GREATEST(1,coalesce((SELECT max(sequence) FROM messages),0)), true);
CREATE UNIQUE INDEX message_sequence ON messages(session_id,sequence);
CREATE TABLE message_revisions (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(), message_id uuid NOT NULL REFERENCES messages(id),
 job_id uuid NOT NULL UNIQUE REFERENCES jobs(id), content text NOT NULL, mood text, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX message_revisions_message ON message_revisions(message_id);
ALTER TABLE payment_orders ADD COLUMN payment_url text;
ALTER TABLE auth_identities ENABLE ROW LEVEL SECURITY;
ALTER TABLE web_auth_sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE auth_flows ENABLE ROW LEVEL SECURITY;
ALTER TABLE credit_grants ENABLE ROW LEVEL SECURITY;
ALTER TABLE request_deduplications ENABLE ROW LEVEL SECURITY;
ALTER TABLE message_revisions ENABLE ROW LEVEL SECURITY;
REVOKE ALL ON auth_identities,web_auth_sessions,auth_flows,credit_grants,request_deduplications,message_revisions FROM PUBLIC;
DO $$ BEGIN
 IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname='anon') THEN
  REVOKE ALL ON auth_identities,web_auth_sessions,auth_flows,credit_grants,request_deduplications,message_revisions FROM anon;
 END IF;
 IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname='authenticated') THEN
  REVOKE ALL ON auth_identities,web_auth_sessions,auth_flows,credit_grants,request_deduplications,message_revisions FROM authenticated;
 END IF;
END $$;


-- web_auth
-- All identity resolution and grants share the same transaction; no matching on email/name.
CREATE FUNCTION web_login(p jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE ident jsonb:=p->'identity'; flow jsonb:=p->'flow'; owner uuid; link_owner uuid:=(flow->>'link_user_id')::uuid; credits integer; grant_added integer;
BEGIN
 PERFORM pg_advisory_xact_lock(hashtextextended((ident->>'provider')||':'||(ident->>'issuer')||':'||(ident->>'subject'),0));
 SELECT user_id INTO owner FROM auth_identities WHERE provider=ident->>'provider' AND issuer=ident->>'issuer' AND subject=ident->>'subject';
 IF owner IS NULL AND ident->>'provider'='line' THEN
  SELECT id INTO owner FROM users WHERE line_user_id=ident->>'subject' FOR UPDATE;
 END IF;
 IF link_owner IS NOT NULL THEN
  PERFORM 1 FROM web_auth_sessions WHERE token_hash=flow->>'link_session_hash' AND user_id=link_owner AND revoked_at IS NULL AND expires_at>now() AND last_seen_at>now()-interval '7 days' FOR UPDATE;
  IF NOT FOUND THEN RETURN jsonb_build_object('error','AUTH_EXPIRED'); END IF;
  IF owner IS NOT NULL AND owner<>link_owner THEN RETURN jsonb_build_object('error','IDENTITY_ALREADY_LINKED'); END IF;
  owner:=link_owner;
 END IF;
 IF owner IS NULL THEN
  IF NOT coalesce((p->>'allow_registration')::boolean,true) THEN RETURN jsonb_build_object('error','ADMISSIONS_DISABLED'); END IF;
  INSERT INTO users(line_user_id,display_name,status,first_surface,acquisition_source) VALUES(CASE WHEN ident->>'provider'='line' THEN ident->>'subject' END, left(ident->>'display_name',255),'inactive','web',coalesce(nullif(flow->>'acquisition_source',''),'direct')) RETURNING id INTO owner;
 END IF;
 PERFORM 1 FROM users WHERE id=owner AND account_status='active' FOR UPDATE;
 IF NOT FOUND THEN RETURN jsonb_build_object('error','ACCOUNT_SUSPENDED'); END IF;
 IF EXISTS(SELECT 1 FROM auth_identities WHERE user_id=owner AND provider=ident->>'provider' AND (issuer<>ident->>'issuer' OR subject<>ident->>'subject')) OR (ident->>'provider'='line' AND EXISTS(SELECT 1 FROM users WHERE id=owner AND line_user_id IS NOT NULL AND line_user_id<>ident->>'subject')) THEN RETURN jsonb_build_object('error','IDENTITY_ALREADY_LINKED'); END IF;
 INSERT INTO auth_identities(provider,issuer,subject,user_id) VALUES(ident->>'provider',ident->>'issuer',ident->>'subject',owner) ON CONFLICT DO NOTHING;
 IF ident->>'provider'='line' THEN UPDATE users SET line_user_id=ident->>'subject' WHERE id=owner; END IF;
 INSERT INTO credit_balances(user_id,balance) VALUES(owner,0) ON CONFLICT(user_id) DO NOTHING;
 SELECT value::integer INTO credits FROM app_config WHERE key='welcome_credits';
 IF credits IS NULL OR credits<0 THEN RAISE EXCEPTION 'Invalid welcome credit config'; END IF;
 INSERT INTO credit_grants(user_id,grant_kind,period_key,amount) VALUES(owner,'welcome','once',credits) ON CONFLICT DO NOTHING;
 GET DIAGNOSTICS grant_added=ROW_COUNT;
 IF grant_added=1 AND credits>0 THEN
  UPDATE credit_balances SET balance=balance+credits,updated_at=now() WHERE user_id=owner;
  INSERT INTO credit_transactions(user_id,type,amount,balance_after,description) SELECT owner,'bonus',credits,balance,'Welcome credits' FROM credit_balances WHERE user_id=owner;
 END IF;
 IF link_owner IS NOT NULL THEN UPDATE web_auth_sessions SET revoked_at=now() WHERE token_hash=flow->>'link_session_hash'; END IF;
 INSERT INTO web_auth_sessions(token_hash,user_id,csrf_token,expires_at) VALUES(p->>'token_hash',owner,p->>'csrf',now()+interval '30 days');
 INSERT INTO analytics_events(user_id,event_name,properties,client_event_id,occurred_at) VALUES(owner,'login_success',jsonb_build_object('surface','web','provider',ident->>'provider'),gen_random_uuid(),now());
 RETURN jsonb_build_object('user_id',owner);
END $$;
REVOKE ALL ON FUNCTION web_login(jsonb) FROM PUBLIC;


-- web_stories
CREATE FUNCTION web_scene(p_id uuid) RETURNS jsonb LANGUAGE sql STABLE AS $$
 SELECT jsonb_build_object('id',s.id,'name',s.name,'location',s.location,'time_of_day',s.time_of_day,'opening_narrator',s.opening_narrator,'opening_dialogue',s.opening_dialogue,'image_url',s.image_url,'suggested_first_replies',s.suggested_first_replies,'is_adult_content',s.is_adult_content,'character',jsonb_build_object('id',c.id,'name',c.name,'avatar_url',c.avatar_url,'personality',c.personality,'tags',c.personality_tags),'cost_per_turn',2)
 FROM scenes s JOIN characters c ON c.id=s.character_id WHERE s.id=p_id AND s.is_active AND c.is_active
$$;
CREATE FUNCTION web_story(p_user uuid,p_id uuid) RETURNS jsonb LANGUAGE sql STABLE AS $$
 SELECT jsonb_build_object('id',s.id,'status',s.status,'interaction_channel',s.interaction_channel,'context_version',s.context_version,'persona',s.persona,'mood',s.mood,'relationship_level',s.relationship_level,'updated_at',s.updated_at,
 'scene',jsonb_build_object('id',c.id,'name',c.name,'image_url',c.image_url,'available',c.is_active AND a.is_active),
 'character',jsonb_build_object('id',a.id,'name',a.name,'avatar_url',a.avatar_url),
 'pending_job',(SELECT jsonb_build_object('id',j.id,'status',j.status,'session_id',j.session_id,'kind',j.kind,'result',j.result,'error',null) FROM jobs j WHERE j.user_id=p_user AND j.mode='roleplay_message' AND j.status IN('pending','processing') LIMIT 1),
 'capabilities',jsonb_build_object('can_send',s.interaction_channel='web' AND s.status='active' AND c.is_active AND a.is_active,'can_transfer_to_web',s.interaction_channel='line' AND c.is_active AND a.is_active,'can_regenerate',s.interaction_channel='web' AND s.status='active' AND c.is_active AND a.is_active AND EXISTS(SELECT 1 FROM messages m WHERE m.session_id=s.id AND m.role='character' AND m.turn_id IS NOT NULL)))
 FROM roleplay_sessions s JOIN scenes c ON c.id=s.scene_id JOIN characters a ON a.id=s.character_id WHERE s.user_id=p_user AND s.id=p_id
$$;
CREATE FUNCTION web_mutate_story(p_user uuid,p_operation text,p jsonb,p_key text) RETURNS jsonb LANGUAGE plpgsql AS $$
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
REVOKE ALL ON FUNCTION web_scene(uuid),web_story(uuid,uuid),web_mutate_story(uuid,text,jsonb,text) FROM PUBLIC;


-- shared_turns
CREATE FUNCTION admit_turn(p_user uuid,p_session uuid,p_origin text,p_kind text,p_key text,p jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
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
CREATE FUNCTION settle_turn(p_job uuid,p_lease uuid,p_response jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE j jobs; s roleplay_sessions; answer_id uuid; user_message_id uuid; balance_now integer; content text:=p_response->>'content'; new_mood text; count_now integer; relation text; target messages;
BEGIN
 -- Admission and settlement use the same owner-first lock order.
 PERFORM 1 FROM users WHERE id=(SELECT user_id FROM jobs WHERE id=p_job) FOR UPDATE;
 SELECT * INTO j FROM jobs WHERE id=p_job FOR UPDATE;
 IF j.status='completed' THEN RETURN j.result; END IF;
 IF j.status<>'processing' OR j.lease_token IS DISTINCT FROM p_lease OR j.lease_expires_at<=now() THEN RETURN jsonb_build_object('error','STALE_LEASE'); END IF;
 SELECT * INTO s FROM roleplay_sessions WHERE id=j.session_id FOR UPDATE;
 IF s.context_version<>j.context_version THEN RETURN jsonb_build_object('error','STALE_VERSION'); END IF;
 IF nullif(btrim(content),'') IS NULL THEN RAISE EXCEPTION 'Empty generation'; END IF;
 new_mood:=coalesce(p_response->>'mood',j.checkpoint->>'mood',s.mood);
 IF j.kind='regeneration' THEN
  SELECT * INTO target FROM messages WHERE id=j.target_message_id AND session_id=s.id FOR UPDATE;
  IF target.sequence<>(SELECT max(sequence) FROM messages WHERE session_id=s.id) THEN RETURN jsonb_build_object('error','INVALID_REGENERATION'); END IF;
  INSERT INTO message_revisions(message_id,job_id,content,mood) VALUES(target.id,j.id,target.content,target.mood);
  UPDATE messages SET content=p_response->>'content',mood=new_mood WHERE id=target.id;
  answer_id:=target.id;
  count_now:=s.message_count; relation:=s.relationship_level;
 ELSE
  INSERT INTO messages(session_id,role,content,turn_id) VALUES(s.id,'user',j.user_message,j.id) RETURNING id INTO user_message_id;
  INSERT INTO messages(session_id,role,content,mood,turn_id) VALUES(s.id,'character',content,new_mood,j.id) RETURNING id INTO answer_id;
  count_now:=s.message_count+1; relation:=s.relationship_level;
  IF relation='stranger' AND count_now >= (SELECT value::int FROM app_config WHERE key='relationship_threshold_acquaintance') THEN relation:='acquaintance';
  ELSIF relation='acquaintance' AND count_now >= (SELECT value::int FROM app_config WHERE key='relationship_threshold_friend') THEN relation:='friend';
  ELSIF relation='friend' AND count_now >= (SELECT value::int FROM app_config WHERE key='relationship_threshold_close_friend') THEN relation:='close_friend'; END IF;
 END IF;
 UPDATE credit_balances SET balance=balance-j.reservation,reserved=reserved-j.reservation,total_consumed=total_consumed+j.reservation,updated_at=now() WHERE user_id=j.user_id RETURNING balance INTO balance_now;
 INSERT INTO credit_transactions(user_id,type,amount,balance_after,reference_id) VALUES(j.user_id,'consumption',-j.reservation,balance_now,j.id);
 UPDATE roleplay_sessions SET mood=new_mood,current_location=coalesce(p_response->>'current_location',j.checkpoint->>'current_location'),scene_time=coalesce(p_response->>'scene_time',j.checkpoint->>'scene_time'),scene_summary=coalesce(p_response->>'scene_summary',CASE WHEN j.kind='regeneration' THEN j.checkpoint->>'scene_summary' ELSE scene_summary END),message_count=count_now,relationship_level=relation,context_version=context_version+1,updated_at=now() WHERE id=s.id;
 UPDATE jobs SET status='completed',completed_at=now(),reservation=0,delivery_status=CASE WHEN origin='line' THEN 'pending' ELSE 'none' END,result=jsonb_build_object('content',p_response->>'content','message_id',answer_id,'user_message_id',user_message_id,'context_version',s.context_version+1),updated_at=now() WHERE id=j.id RETURNING result INTO j.result;
 INSERT INTO analytics_events(user_id,event_name,properties,client_event_id,occurred_at) VALUES(j.user_id,CASE WHEN j.kind='regeneration' THEN 'regeneration_completed' ELSE 'turn_completed' END,jsonb_build_object('surface',j.origin,'session_id',j.session_id,'first_turn',count_now=1,'duration_ms',extract(epoch from (now()-j.created_at))*1000),j.id,now()) ON CONFLICT DO NOTHING;
 RETURN j.result;
END $$;
CREATE FUNCTION fail_turn(p_job uuid,p_lease uuid) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE j jobs;
BEGIN
 PERFORM 1 FROM users WHERE id=(SELECT user_id FROM jobs WHERE id=p_job) FOR UPDATE;
 SELECT * INTO j FROM jobs WHERE id=p_job FOR UPDATE;
 IF j.status NOT IN('pending','processing') OR j.lease_token IS DISTINCT FROM p_lease THEN RETURN false; END IF;
 UPDATE credit_balances SET reserved=reserved-j.reservation WHERE user_id=j.user_id;
 UPDATE jobs SET status='failed',reservation=0,delivery_status=CASE WHEN origin='line' THEN 'pending' ELSE 'none' END,failed_reason='GENERATION_FAILED',completed_at=now(),updated_at=now() WHERE id=j.id;
 RETURN true;
END $$;
REVOKE ALL ON FUNCTION admit_turn(uuid,uuid,text,text,text,jsonb),settle_turn(uuid,uuid,jsonb),fail_turn(uuid,uuid) FROM PUBLIC;


-- payment_and_delivery
CREATE UNIQUE INDEX payment_purchase_once ON credit_transactions(reference_id) WHERE type='purchase' AND reference_id IS NOT NULL;
CREATE UNIQUE INDEX beam_link_unique ON payment_orders(beam_payment_link_id) WHERE beam_payment_link_id<>'';
ALTER TABLE jobs ADD COLUMN delivery_lease_token uuid;
ALTER TABLE jobs ADD COLUMN delivery_first_attempt_at timestamptz;
ALTER TABLE jobs ADD COLUMN delivery_attempts integer NOT NULL DEFAULT 0;
ALTER TABLE jobs ADD COLUMN delivery_retry_at timestamptz;
CREATE FUNCTION settle_payment(p_id uuid,p_status text) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE o payment_orders; new_balance integer;
BEGIN
 SELECT * INTO o FROM payment_orders WHERE id=p_id FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'Payment not found'; END IF;
 IF o.status='completed' THEN RETURN false; END IF;
 UPDATE credit_balances SET balance=balance+o.credits_amount,total_purchased=total_purchased+o.credits_amount,updated_at=now() WHERE user_id=o.user_id RETURNING balance INTO new_balance;
 IF NOT FOUND THEN RAISE EXCEPTION 'Wallet missing'; END IF;
 INSERT INTO credit_transactions(user_id,type,amount,balance_after,reference_id,description) VALUES(o.user_id,'purchase',o.credits_amount,new_balance,o.id,'Credit purchase');
 UPDATE payment_orders SET status='completed',beam_status=p_status,updated_at=now() WHERE id=o.id;
 INSERT INTO analytics_events(user_id,event_name,properties,client_event_id,occurred_at) VALUES(o.user_id,'payment_completed',jsonb_build_object('order_id',o.id,'credits',o.credits_amount),o.id,now()) ON CONFLICT DO NOTHING;
 RETURN true;
END $$;
CREATE FUNCTION web_payment_order(p_user uuid,p_key text,p_package text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE existing request_deduplications; id uuid:=gen_random_uuid(); credits integer; price integer;
BEGIN
 PERFORM 1 FROM users WHERE users.id=p_user AND account_status='active' FOR UPDATE;
 IF NOT FOUND THEN RETURN jsonb_build_object('error','ACCOUNT_SUSPENDED'); END IF;
 SELECT * INTO existing FROM request_deduplications WHERE user_id=p_user AND operation='payment' AND request_key=p_key;
 IF FOUND THEN
  IF existing.payload<>to_jsonb(p_package) THEN RETURN jsonb_build_object('error','IDEMPOTENCY_MISMATCH'); END IF;
  RETURN (SELECT jsonb_build_object('order_id',p.id,'payment_url',p.payment_url,'is_new',false,'credits',credits_amount,'amount_thb',price_thb) FROM payment_orders p WHERE p.id=existing.result_id);
 END IF;
 CASE p_package WHEN 'basic' THEN credits:=50;price:=29; WHEN 'plus' THEN credits:=150;price:=69; WHEN 'premium' THEN credits:=400;price:=149; ELSE RETURN jsonb_build_object('error','VALIDATION_ERROR'); END CASE;
 INSERT INTO payment_orders(id,user_id,beam_payment_link_id,package_id,credits_amount,price_thb) VALUES(id,p_user,'',p_package,credits,price);
 INSERT INTO request_deduplications(user_id,operation,request_key,payload,result_id) VALUES(p_user,'payment',p_key,to_jsonb(p_package),id);
 RETURN jsonb_build_object('order_id',id,'payment_url',null,'is_new',true,'credits',credits,'amount_thb',price);
END $$;
CREATE FUNCTION protect_pending_story() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF (OLD.status<>NEW.status OR OLD.interaction_channel<>NEW.interaction_channel OR OLD.persona<>NEW.persona) AND EXISTS(SELECT 1 FROM jobs WHERE session_id=OLD.id AND mode='roleplay_message' AND status IN('pending','processing')) THEN RAISE EXCEPTION 'TURN_IN_PROGRESS'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER protect_pending_story BEFORE UPDATE ON roleplay_sessions FOR EACH ROW EXECUTE FUNCTION protect_pending_story();
REVOKE ALL ON FUNCTION settle_payment(uuid,text),web_payment_order(uuid,text,text),protect_pending_story() FROM PUBLIC;


-- web_permissions
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM pg_roles WHERE rolname='service_role') THEN
  GRANT ALL ON auth_identities,web_auth_sessions,auth_flows,credit_grants,request_deduplications,message_revisions TO service_role;
  GRANT EXECUTE ON FUNCTION web_login(jsonb),web_scene(uuid),web_story(uuid,uuid),web_mutate_story(uuid,text,jsonb,text),admit_turn(uuid,uuid,text,text,text,jsonb),settle_turn(uuid,uuid,jsonb),fail_turn(uuid,uuid),settle_payment(uuid,text),web_payment_order(uuid,text,text) TO service_role;
 END IF;
END $$;

-- Existing deployments may not yet have the weekly configuration from migration 021.
INSERT INTO app_config(key,value) VALUES('daily_checkin_weekly_credits','2,3,4,4,4,4,10') ON CONFLICT(key) DO NOTHING;

-- Created last so startup/readiness cannot accept a partially applied schema.
CREATE FUNCTION talkrai_schema_version() RETURNS integer LANGUAGE sql STABLE AS $$ SELECT 23 $$;
REVOKE ALL ON FUNCTION talkrai_schema_version() FROM PUBLIC;
DO $$
DECLARE fn regprocedure; client_role text;
BEGIN
 FOR fn IN SELECT p.oid::regprocedure FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
 WHERE n.nspname='public' AND p.proname IN ('web_login','web_scene','web_story','web_mutate_story','admit_turn','settle_turn','fail_turn','settle_payment','web_payment_order','protect_pending_story','talkrai_schema_version') LOOP
  FOREACH client_role IN ARRAY ARRAY['anon','authenticated'] LOOP
   IF EXISTS(SELECT 1 FROM pg_roles WHERE rolname=client_role) THEN EXECUTE format('REVOKE ALL ON FUNCTION %s FROM %I',fn,client_role); END IF;
  END LOOP;
 END LOOP;
 IF EXISTS(SELECT 1 FROM pg_roles WHERE rolname='service_role') THEN GRANT EXECUTE ON FUNCTION talkrai_schema_version() TO service_role; END IF;
END $$;
