INSERT INTO app_config (key, value) VALUES
    ('narrator_avatar_url', 'https://example.com/narrator-avatar.png'),
    ('narrator_display_name', E'\u3164'),
    ('rich_menu_no_session', 'richmenu-xxx-state-a'),
    ('rich_menu_active_session', 'richmenu-xxx-state-b'),
    ('ai_max_tokens', '1024'),
    ('welcome_credits', '50'),
    ('relationship_threshold_acquaintance', '20'),
    ('relationship_threshold_friend', '40'),
    ('relationship_threshold_close_friend', '60')
ON CONFLICT (key) DO NOTHING;
