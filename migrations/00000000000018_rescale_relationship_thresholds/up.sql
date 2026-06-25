-- Phase 0 (spec 2026-06-24 §C.8) — rescale the relationship thresholds for the activation cliffhanger.
--
-- The free budget is exactly 10 messages (welcome_credits 20 ÷ 2 credits/message). Landing the first
-- level-up (Stranger → Acquaintance) at message_count 8 puts the "aha" just *inside* the free credits,
-- one or two warmer messages before the wall (peak-end). Friend/CloseFriend stay reachable via daily
-- refills, with CloseFriend the deep tier where paying lives.
--
-- Upsert (not a bare UPDATE) so this is idempotent and self-healing: it sets the new value whether the
-- key already exists (existing databases seeded by 00000000000008) or is somehow absent. The seed
-- migration's `INSERT ... ON CONFLICT DO NOTHING` never overwrites an already-seeded row, so a new
-- migration is required to move the values on an existing deployment.
INSERT INTO app_config (key, value) VALUES
    ('relationship_threshold_acquaintance', '8'),
    ('relationship_threshold_friend',       '24'),
    ('relationship_threshold_close_friend', '70')
ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = now();
