-- Read-only launch monitoring. Revenue is gross THB, not profit;
-- join provider usage/invoices separately before reporting contribution margin.
BEGIN READ ONLY;

-- Weekly engaged players: canonical users with >=10 actual user messages;
-- regeneration replaces an answer and cannot inflate this count.
WITH engaged AS (
 SELECT s.user_id,count(*) AS user_messages
 FROM messages m JOIN roleplay_sessions s ON s.id=m.session_id
 WHERE m.role='user' AND m.created_at>=now()-interval '7 days'
 GROUP BY s.user_id HAVING count(*)>=10
)
SELECT u.first_surface,u.acquisition_source,count(*) AS weekly_engaged_players
FROM engaged e JOIN users u ON u.id=e.user_id GROUP BY u.first_surface,u.acquisition_source;

-- Generation outcomes, latency and regeneration mix over the last seven days.
SELECT origin,kind,status,count(*) AS jobs,
 percentile_cont(0.5) WITHIN GROUP (ORDER BY extract(epoch FROM completed_at-created_at)) FILTER(WHERE status='completed') AS p50_seconds,
 percentile_cont(0.95) WITHIN GROUP (ORDER BY extract(epoch FROM completed_at-created_at)) FILTER(WHERE status='completed') AS p95_seconds
FROM jobs WHERE context_version IS NOT NULL AND created_at>=now()-interval '7 days'
GROUP BY origin,kind,status;

-- Payment conversion counts and collected gross value by initial surface.
SELECT u.first_surface,count(*) AS orders,
 count(*) FILTER(WHERE p.status='completed') AS paid_orders,
 coalesce(sum(p.price_thb) FILTER(WHERE p.status='completed'),0) AS gross_thb
FROM payment_orders p JOIN users u ON u.id=p.user_id
WHERE p.created_at>=now()-interval '7 days' GROUP BY u.first_surface;

-- Wallet invariant monitor; a non-zero row count requires investigation.
SELECT b.user_id,b.reserved,coalesce(sum(j.reservation) FILTER(WHERE j.status IN('pending','processing')),0) AS active_reservations
FROM credit_balances b LEFT JOIN jobs j ON j.user_id=b.user_id AND j.context_version IS NOT NULL
GROUP BY b.user_id,b.reserved
HAVING b.reserved<>coalesce(sum(j.reservation) FILTER(WHERE j.status IN('pending','processing')),0);

SELECT status,delivery_status,count(*) AS jobs,min(created_at) AS oldest
FROM jobs WHERE context_version IS NOT NULL AND (status IN('pending','processing') OR delivery_status='failed')
GROUP BY status,delivery_status;
ROLLBACK;
