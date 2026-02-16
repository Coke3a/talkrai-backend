DELETE FROM app_config WHERE key IN (
    'narrator_avatar_url', 'narrator_display_name',
    'rich_menu_no_session', 'rich_menu_active_session',
    'ai_max_tokens', 'welcome_credits'
);
