-- ============================================================================
-- Migration: Enable Row Level Security (RLS) for all tables
-- Purpose: Defense-in-depth + future admin dashboard support
--
-- Architecture:
--   - Rust backend uses service_role (bypasses RLS)
--   - RLS protects against direct client access
--   - Admin policies ready for future Supabase Auth integration
--
-- Supabase roles:
--   - anon: no access to any table
--   - authenticated: user-scoped access via auth.uid()
--   - service_role: full access (bypasses RLS)
-- ============================================================================

-- ============================================================================
-- 1. SCHEMA CHANGE: Add role column to users
-- ============================================================================

ALTER TABLE users
  ADD COLUMN role VARCHAR NOT NULL DEFAULT 'user'
  CONSTRAINT users_role_check CHECK (role IN ('user', 'admin'));

-- Composite index for admin check in is_admin() helper function
CREATE INDEX idx_users_id_role ON users (id, role);

-- ============================================================================
-- 2. HELPER FUNCTIONS (SECURITY DEFINER)
-- ============================================================================

-- Check if the current authenticated user is an admin
-- Uses (select auth.uid()) pattern for performance (called once, not per row)
CREATE OR REPLACE FUNCTION public.is_admin()
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT EXISTS (
    SELECT 1 FROM public.users
    WHERE id = (SELECT auth.uid())
      AND role = 'admin'
  );
$$;

-- Check if the current authenticated user owns a specific session
-- Used by messages table policies to verify access through session ownership
CREATE OR REPLACE FUNCTION public.user_owns_session(p_session_id UUID)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT EXISTS (
    SELECT 1 FROM public.roleplay_sessions
    WHERE id = p_session_id
      AND user_id = (SELECT auth.uid())
  );
$$;

-- ============================================================================
-- 3. ENABLE ROW LEVEL SECURITY ON ALL TABLES
-- ============================================================================

ALTER TABLE users ENABLE ROW LEVEL SECURITY;
ALTER TABLE characters ENABLE ROW LEVEL SECURITY;
ALTER TABLE scenes ENABLE ROW LEVEL SECURITY;
ALTER TABLE roleplay_sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE messages ENABLE ROW LEVEL SECURITY;
ALTER TABLE jobs ENABLE ROW LEVEL SECURITY;
ALTER TABLE credit_balances ENABLE ROW LEVEL SECURITY;
ALTER TABLE credit_transactions ENABLE ROW LEVEL SECURITY;
ALTER TABLE app_config ENABLE ROW LEVEL SECURITY;

-- ============================================================================
-- 4. REVOKE DEFAULT PUBLIC ACCESS & GRANT ROLE PERMISSIONS
-- ============================================================================

-- Revoke all default access from anon and authenticated
REVOKE ALL ON ALL TABLES IN SCHEMA public FROM anon;
REVOKE ALL ON ALL TABLES IN SCHEMA public FROM authenticated;

-- Ensure future tables also have no default access
ALTER DEFAULT PRIVILEGES IN SCHEMA public REVOKE ALL ON TABLES FROM anon;
ALTER DEFAULT PRIVILEGES IN SCHEMA public REVOKE ALL ON TABLES FROM authenticated;

-- Grant SELECT on reference/public tables to authenticated
GRANT SELECT ON users TO authenticated;
GRANT SELECT ON characters TO authenticated;
GRANT SELECT ON scenes TO authenticated;
GRANT SELECT ON roleplay_sessions TO authenticated;
GRANT SELECT ON messages TO authenticated;
GRANT SELECT ON jobs TO authenticated;
GRANT SELECT ON credit_balances TO authenticated;
GRANT SELECT ON credit_transactions TO authenticated;
GRANT SELECT ON app_config TO authenticated;

-- Grant column-level UPDATE on users (exclude 'role' to prevent escalation)
GRANT UPDATE (display_name, picture_url, language, terms_accepted_at) ON users TO authenticated;
GRANT INSERT, UPDATE ON roleplay_sessions TO authenticated;
GRANT INSERT ON messages TO authenticated;

-- Admin-writable tables (authenticated role, but RLS restricts to admins)
GRANT INSERT, UPDATE, DELETE ON characters TO authenticated;
GRANT INSERT, UPDATE, DELETE ON scenes TO authenticated;
GRANT INSERT, UPDATE, DELETE ON app_config TO authenticated;

-- Grant function execution to authenticated
GRANT EXECUTE ON FUNCTION public.is_admin() TO authenticated;
GRANT EXECUTE ON FUNCTION public.user_owns_session(UUID) TO authenticated;

-- ============================================================================
-- 5. RLS POLICIES: users
-- ============================================================================

-- Users can read their own profile; admins can read all
CREATE POLICY users_select_own ON users
  FOR SELECT
  TO authenticated
  USING (
    id = (SELECT auth.uid())
    OR (SELECT public.is_admin())
  );

-- Users can update their own profile
-- Note: 'role' column is protected via column-level GRANT (line 101)
CREATE POLICY users_update_own ON users
  FOR UPDATE
  TO authenticated
  USING (id = (SELECT auth.uid()))
  WITH CHECK (id = (SELECT auth.uid()));

-- ============================================================================
-- 6. RLS POLICIES: characters (shared/public content)
-- ============================================================================

-- All authenticated users can read active characters
CREATE POLICY characters_select_active ON characters
  FOR SELECT
  TO authenticated
  USING (
    is_active = true
    OR (SELECT public.is_admin())
  );

-- Only admins can insert characters
CREATE POLICY characters_insert_admin ON characters
  FOR INSERT
  TO authenticated
  WITH CHECK ((SELECT public.is_admin()));

-- Only admins can update characters
CREATE POLICY characters_update_admin ON characters
  FOR UPDATE
  TO authenticated
  USING ((SELECT public.is_admin()))
  WITH CHECK ((SELECT public.is_admin()));

-- Only admins can delete characters
CREATE POLICY characters_delete_admin ON characters
  FOR DELETE
  TO authenticated
  USING ((SELECT public.is_admin()));

-- ============================================================================
-- 7. RLS POLICIES: scenes (shared/public content)
-- ============================================================================

-- All authenticated users can read active scenes
CREATE POLICY scenes_select_active ON scenes
  FOR SELECT
  TO authenticated
  USING (
    is_active = true
    OR (SELECT public.is_admin())
  );

-- Only admins can insert scenes
CREATE POLICY scenes_insert_admin ON scenes
  FOR INSERT
  TO authenticated
  WITH CHECK ((SELECT public.is_admin()));

-- Only admins can update scenes
CREATE POLICY scenes_update_admin ON scenes
  FOR UPDATE
  TO authenticated
  USING ((SELECT public.is_admin()))
  WITH CHECK ((SELECT public.is_admin()));

-- Only admins can delete scenes
CREATE POLICY scenes_delete_admin ON scenes
  FOR DELETE
  TO authenticated
  USING ((SELECT public.is_admin()));

-- ============================================================================
-- 8. RLS POLICIES: roleplay_sessions
-- ============================================================================

-- Users can read their own sessions; admins can read all
CREATE POLICY sessions_select_own ON roleplay_sessions
  FOR SELECT
  TO authenticated
  USING (
    user_id = (SELECT auth.uid())
    OR (SELECT public.is_admin())
  );

-- Users can create sessions for themselves only
CREATE POLICY sessions_insert_own ON roleplay_sessions
  FOR INSERT
  TO authenticated
  WITH CHECK (user_id = (SELECT auth.uid()));

-- Users can update their own sessions; admins can update any
CREATE POLICY sessions_update_own ON roleplay_sessions
  FOR UPDATE
  TO authenticated
  USING (
    user_id = (SELECT auth.uid())
    OR (SELECT public.is_admin())
  )
  WITH CHECK (
    user_id = (SELECT auth.uid())
    OR (SELECT public.is_admin())
  );

-- ============================================================================
-- 9. RLS POLICIES: messages (immutable - no UPDATE/DELETE)
-- ============================================================================

-- Users can read messages from their own sessions; admins can read all
CREATE POLICY messages_select_own ON messages
  FOR SELECT
  TO authenticated
  USING (
    (SELECT public.user_owns_session(session_id))
    OR (SELECT public.is_admin())
  );

-- Users can insert messages into their own sessions
CREATE POLICY messages_insert_own ON messages
  FOR INSERT
  TO authenticated
  WITH CHECK ((SELECT public.user_owns_session(session_id)));

-- ============================================================================
-- 10. RLS POLICIES: jobs (service-only write, admin read)
-- ============================================================================

-- Only admins can read jobs (for monitoring/debugging)
CREATE POLICY jobs_select_admin ON jobs
  FOR SELECT
  TO authenticated
  USING ((SELECT public.is_admin()));

-- No INSERT/UPDATE/DELETE policies for authenticated role
-- Jobs are created and processed by backend via service_role

-- ============================================================================
-- 11. RLS POLICIES: credit_balances (user read-only)
-- ============================================================================

-- Users can read their own balance; admins can read all
CREATE POLICY credit_balances_select_own ON credit_balances
  FOR SELECT
  TO authenticated
  USING (
    user_id = (SELECT auth.uid())
    OR (SELECT public.is_admin())
  );

-- No INSERT/UPDATE/DELETE policies for authenticated role
-- Credit balances are managed by backend via service_role

-- ============================================================================
-- 12. RLS POLICIES: credit_transactions (user read-only)
-- ============================================================================

-- Users can read their own transactions; admins can read all
CREATE POLICY credit_transactions_select_own ON credit_transactions
  FOR SELECT
  TO authenticated
  USING (
    user_id = (SELECT auth.uid())
    OR (SELECT public.is_admin())
  );

-- No INSERT/UPDATE/DELETE policies for authenticated role
-- Credit transactions are created by backend via service_role

-- ============================================================================
-- 13. RLS POLICIES: app_config
-- ============================================================================

-- All authenticated users can read configuration
CREATE POLICY app_config_select_all ON app_config
  FOR SELECT
  TO authenticated
  USING (true);

-- Only admins can insert config
CREATE POLICY app_config_insert_admin ON app_config
  FOR INSERT
  TO authenticated
  WITH CHECK ((SELECT public.is_admin()));

-- Only admins can update config
CREATE POLICY app_config_update_admin ON app_config
  FOR UPDATE
  TO authenticated
  USING ((SELECT public.is_admin()))
  WITH CHECK ((SELECT public.is_admin()));

-- Only admins can delete config
CREATE POLICY app_config_delete_admin ON app_config
  FOR DELETE
  TO authenticated
  USING ((SELECT public.is_admin()));
