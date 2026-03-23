-- ============================================================================
-- Rollback Migration 010: Revert tag system back to genre_tags
-- ============================================================================

-- Drop RLS policies for tag_definitions
DROP POLICY IF EXISTS tag_definitions_select_active ON tag_definitions;
DROP POLICY IF EXISTS tag_definitions_insert_admin ON tag_definitions;
DROP POLICY IF EXISTS tag_definitions_update_admin ON tag_definitions;
DROP POLICY IF EXISTS tag_definitions_delete_admin ON tag_definitions;

-- Revoke permissions
REVOKE ALL ON tag_definitions FROM authenticated;

-- Drop tag_definitions table
DROP TABLE IF EXISTS tag_definitions;

-- Drop new indexes on characters
DROP INDEX IF EXISTS idx_characters_appearance_tags;
DROP INDEX IF EXISTS idx_characters_personality_tags;

-- Restore genre_tags column and drop new columns
ALTER TABLE characters
    ADD COLUMN genre_tags TEXT[] NOT NULL DEFAULT '{}';

ALTER TABLE characters
    DROP COLUMN appearance_tags,
    DROP COLUMN personality_tags;
