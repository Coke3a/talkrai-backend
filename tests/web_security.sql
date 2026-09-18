BEGIN;
DO $$
DECLARE owner uuid; result jsonb; fn regprocedure;
BEGIN
 result:=web_login('{"identity":{"provider":"line","issuer":"https://access.line.me","subject":"security-line-one","display_name":"Test"},"flow":{},"token_hash":"security-token-one","csrf":"test"}');
 owner:=(result->>'user_id')::uuid;
 result:=web_login(jsonb_build_object('identity',jsonb_build_object('provider','line','issuer','https://access.line.me','subject','security-line-two','display_name','Test'),'flow',jsonb_build_object('link_user_id',owner,'link_session_hash','security-token-one'),'token_hash','security-token-two','csrf','test'));
 IF result->>'error' IS DISTINCT FROM 'IDENTITY_ALREADY_LINKED' THEN RAISE EXCEPTION 'A second LINE identity must be rejected'; END IF;
 IF (SELECT line_user_id FROM users WHERE id=owner)<>'security-line-one' THEN RAISE EXCEPTION 'Original LINE identity changed'; END IF;
 IF (SELECT count(*) FROM auth_identities WHERE user_id=owner AND provider='line')<>1 THEN RAISE EXCEPTION 'Multiple LINE identities'; END IF;
 IF (SELECT revoked_at FROM web_auth_sessions WHERE token_hash='security-token-one') IS NOT NULL THEN RAISE EXCEPTION 'Rejected link revoked valid session'; END IF;
 result:=web_login(jsonb_build_object('identity',jsonb_build_object('provider','google','issuer','https://accounts.google.com','subject','security-google','display_name','Test'),'flow',jsonb_build_object('link_user_id',owner,'link_session_hash','security-token-one'),'token_hash','security-token-google','csrf','test'));
 IF (result->>'user_id')::uuid IS DISTINCT FROM owner THEN RAISE EXCEPTION 'Google linking should preserve account'; END IF;
 FOR fn IN SELECT p.oid::regprocedure FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname='public' AND p.proname IN ('web_login','web_scene','web_story','web_mutate_story','admit_turn','settle_turn','fail_turn','settle_payment','web_payment_order','protect_pending_story') LOOP
  IF has_function_privilege('anon',fn,'EXECUTE') OR has_function_privilege('authenticated',fn,'EXECUTE') THEN RAISE EXCEPTION 'Client role may execute backend-only function %',fn; END IF;
 END LOOP;
END $$;
ROLLBACK;
