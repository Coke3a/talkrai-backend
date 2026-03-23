-- ============================================================================
-- Rollback Migration 011: Remove seeded data
-- ============================================================================

-- Reset all character tags to empty
UPDATE characters SET appearance_tags = '{}', personality_tags = '{}';

-- Remove all tag definitions
DELETE FROM tag_definitions;
