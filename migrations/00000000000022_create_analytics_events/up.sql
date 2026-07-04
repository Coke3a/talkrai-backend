-- ============================================================================
-- Migration 022: analytics_events (first-party LIFF behavioral tracking)
--
-- Purpose:
--   Capture client-side signals we cannot otherwise see (LIFF page entries +
--   key button taps) in an append-only log, join-able with users /
--   roleplay_sessions / messages / payment_orders to build an activation funnel.
--
-- Design notes:
--   - PK is BIGINT identity, not UUID (D3): an internal append-only log never
--     exposed in URLs/API; sequential ids keep index writes cheap at high write
--     volume. Deliberate, scoped deviation from the UUID-everywhere convention.
--   - event_name is an open VARCHAR (new event types ship from the frontend with
--     no backend deploy); page is validated app-side against a closed enum.
--   - properties (JSONB) carries IDs/enums only — NEVER free-text / PII.
--   - Dedup anchor: unique client_event_id + ON CONFLICT DO NOTHING at insert.
--
-- RLS (matches the `jobs` table model from migration 009 — service-only write,
-- admin read): the Rust backend uses service_role, which bypasses RLS, so writes
-- work with no INSERT policy. Migration 009 REVOKEd default privileges from
-- `authenticated` for all future tables, so an explicit GRANT SELECT is required
-- for the admin-read policy (via public.is_admin()) to be usable by a future
-- Supabase-Auth admin dashboard.
-- ============================================================================

CREATE TABLE analytics_events (
    id              BIGINT      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id         UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    event_name      VARCHAR(64) NOT NULL,          -- 'page_view' | 'session_start_click' | ...
    page            VARCHAR(32),                   -- one of the 5 LIFF pages; NULL for non-page_view
    properties      JSONB,                         -- ids/enums only, NO free-text / PII
    client_event_id UUID        NOT NULL,          -- client-generated; dedup anchor
    occurred_at     TIMESTAMPTZ NOT NULL,          -- client event time
    received_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX ux_analytics_events_client_event_id ON analytics_events (client_event_id);
CREATE INDEX        ix_analytics_events_user            ON analytics_events (user_id, occurred_at);
CREATE INDEX        ix_analytics_events_name            ON analytics_events (event_name, occurred_at);

-- ============================================================================
-- RLS: service-only write, admin read (mirrors the `jobs` table policy)
-- ============================================================================

ALTER TABLE analytics_events ENABLE ROW LEVEL SECURITY;

-- Only admins can read events (for future monitoring / funnel dashboard).
-- Backend writes go through service_role, which bypasses RLS entirely.
CREATE POLICY analytics_events_select_admin ON analytics_events
    FOR SELECT
    TO authenticated
    USING ((SELECT public.is_admin()));

-- No INSERT/UPDATE/DELETE policies for authenticated — events are written only
-- by the backend via service_role.

-- Table-level grant required because migration 009 revoked default privileges
-- for future tables; RLS still restricts visible rows to admins.
GRANT SELECT ON analytics_events TO authenticated;
