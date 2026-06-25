-- Revert the relationship thresholds to their original (pre-cliffhanger) values.
INSERT INTO app_config (key, value) VALUES
    ('relationship_threshold_acquaintance', '20'),
    ('relationship_threshold_friend',       '40'),
    ('relationship_threshold_close_friend', '60')
ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = now();
