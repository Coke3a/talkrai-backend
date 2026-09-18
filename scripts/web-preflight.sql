-- Run read-only against the intended deployment database before applying migrations.
BEGIN TRANSACTION READ ONLY;
SELECT user_id,count(*) AS active_line_stories FROM roleplay_sessions WHERE status='active' GROUP BY user_id HAVING count(*)>1;
SELECT user_id,count(*) AS active_generations FROM jobs WHERE mode='roleplay_message' AND status IN('pending','processing') GROUP BY user_id;
SELECT type,reference_id,count(*) FROM credit_transactions WHERE type IN('purchase','consumption') AND reference_id IS NOT NULL GROUP BY type,reference_id HAVING count(*)>1;
SELECT beam_payment_link_id,count(*) FROM payment_orders WHERE beam_payment_link_id<>'' GROUP BY beam_payment_link_id HAVING count(*)>1;
SELECT status,count(*) FROM users GROUP BY status;
SELECT key,value FROM app_config WHERE key IN('welcome_credits','daily_checkin_weekly_credits','summarize_interval','ai_max_tokens');
ROLLBACK;
-- Investigate any duplicate rows and the provenance of inactive users. Never auto-delete them.
