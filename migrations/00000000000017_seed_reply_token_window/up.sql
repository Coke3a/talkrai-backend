-- Reply-token validity window (seconds) for choosing reply (free) vs push.
-- Optional override; the code default is DEFAULT_REPLY_TOKEN_WINDOW_SECS (50).
INSERT INTO app_config (key, value) VALUES ('reply_token_window_secs', '50')
ON CONFLICT (key) DO NOTHING;
