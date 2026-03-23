-- ============================================================================
-- Migration 010: Tag Definitions + Character Tag Columns (Hybrid Approach)
--
-- Purpose:
--   Replace free-text `genre_tags` on characters with a normalized tag system
--   for powering suggestion/filter features on the scenes page.
--
-- Architecture (Hybrid):
--   - `tag_definitions` table = source of truth for all valid tags
--   - `characters.appearance_tags` TEXT[] = stores tag keys (validated at app layer)
--   - `characters.personality_tags` TEXT[] = stores tag keys (validated at app layer)
--   - Old `genre_tags` column is dropped
--
-- Tag categories:
--   - appearance: visual/physical traits (cute, cool, elegant, sporty, etc.)
--   - personality: behavioral/character traits (tsundere, cheerful, caring, etc.)
-- ============================================================================

-- ============================================================================
-- 1. CREATE tag_definitions TABLE
-- ============================================================================

CREATE TABLE tag_definitions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    category    VARCHAR(20) NOT NULL CHECK (category IN ('appearance', 'personality')),
    key         VARCHAR(50) NOT NULL UNIQUE,
    display_name VARCHAR(50) NOT NULL,
    description TEXT,
    sort_order  INT NOT NULL DEFAULT 0,
    is_active   BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Index for fast lookups by category + active status
CREATE INDEX idx_tag_definitions_category_active
    ON tag_definitions (category, sort_order)
    WHERE is_active = TRUE;

-- Unique constraint on (category, key) for extra safety
CREATE UNIQUE INDEX idx_tag_definitions_category_key
    ON tag_definitions (category, key);

-- ============================================================================
-- 2. ALTER characters TABLE: add new columns, drop old column
-- ============================================================================

-- Add new typed tag columns
ALTER TABLE characters
    ADD COLUMN appearance_tags TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN personality_tags TEXT[] NOT NULL DEFAULT '{}';

-- Create GIN indexes for fast array containment queries (e.g. WHERE 'cool' = ANY(appearance_tags))
CREATE INDEX idx_characters_appearance_tags ON characters USING GIN (appearance_tags);
CREATE INDEX idx_characters_personality_tags ON characters USING GIN (personality_tags);

-- Drop the old free-text genre_tags column
ALTER TABLE characters DROP COLUMN genre_tags;

-- ============================================================================
-- 3. RLS POLICIES for tag_definitions
-- ============================================================================

ALTER TABLE tag_definitions ENABLE ROW LEVEL SECURITY;

-- All authenticated users can read active tags (for filter UI)
CREATE POLICY tag_definitions_select_active ON tag_definitions
    FOR SELECT
    TO authenticated
    USING (
        is_active = true
        OR (SELECT public.is_admin())
    );

-- Only admins can manage tags
CREATE POLICY tag_definitions_insert_admin ON tag_definitions
    FOR INSERT
    TO authenticated
    WITH CHECK ((SELECT public.is_admin()));

CREATE POLICY tag_definitions_update_admin ON tag_definitions
    FOR UPDATE
    TO authenticated
    USING ((SELECT public.is_admin()))
    WITH CHECK ((SELECT public.is_admin()));

CREATE POLICY tag_definitions_delete_admin ON tag_definitions
    FOR DELETE
    TO authenticated
    USING ((SELECT public.is_admin()));

-- Grant permissions
GRANT SELECT ON tag_definitions TO authenticated;
GRANT INSERT, UPDATE, DELETE ON tag_definitions TO authenticated;
