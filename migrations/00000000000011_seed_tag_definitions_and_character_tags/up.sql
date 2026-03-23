-- ============================================================================
-- Migration 011: Seed tag_definitions + assign tags to existing characters
--
-- This is DATA-ONLY — no schema changes.
-- Separated from migration 010 (schema) for clean rollback/re-seed.
-- ============================================================================

-- ============================================================================
-- 1. SEED tag_definitions: Appearance tags
-- ============================================================================

INSERT INTO tag_definitions (category, key, display_name, description, sort_order) VALUES
    ('appearance', 'cute',          'น่ารัก',      'หน้าอ่อนหวาน ตาโต ดูน่าเอ็นดู',                1),
    ('appearance', 'cool',          'เท่',         'หน้าคม ออร่าเย็น ดูเฉียบ',                     2),
    ('appearance', 'elegant',       'สง่างาม',     'ดูแพง หรูหรา มีออร่า',                         3),
    ('appearance', 'sporty',        'สปอร์ตี้',     'หุ่นแอธเลติก กระฉับกระเฉง',                    4),
    ('appearance', 'gentle_look',   'อ่อนโยน',     'ดูนุ่มนวล น่าไว้ใจ อบอุ่น',                    5),
    ('appearance', 'wild',          'ดุดัน',        'ดูอันตราย ดิบ มีพิษสง',                        6),
    ('appearance', 'intellectual',  'ปัญญาชน',     'ดูฉลาด มีสติปัญญา มีออร่านักคิด',              7);

-- ============================================================================
-- 2. SEED tag_definitions: Personality tags
-- ============================================================================

INSERT INTO tag_definitions (category, key, display_name, description, sort_order) VALUES
    ('personality', 'tsundere',     'ซึนเดเระ',    'ภายนอกเย็นชา/ดุ แต่ข้างในอ่อนโยน',            1),
    ('personality', 'cheerful',     'ร่าเริง',      'สดใส มีพลัง ยิ้มง่ายหัวเราะง่าย',               2),
    ('personality', 'caring',       'อบอุ่น',       'ใจดี เอาใจใส่ ดูแลคนรอบข้าง',                  3),
    ('personality', 'mysterious',   'ลึกลับ',       'พูดน้อย มีเรื่องซ่อน อ่านยาก',                  4),
    ('personality', 'shy',          'ขี้อาย',       'เงียบ พูดน้อย ประหม่าง่าย',                     5),
    ('personality', 'confident',    'มั่นใจ',       'กล้าหาญ ไม่แคร์สายตาคนอื่น',                  6),
    ('personality', 'flirty',       'มีเสน่ห์',     'ดึงดูดคนเป็นธรรมชาติ เจ้าเล่ห์นิดๆ',           7),
    ('personality', 'serious',      'จริงจัง',      'เข้มงวด มีมาตรฐานสูง ไม่เล่นๆ',               8),
    ('personality', 'sensitive',    'อ่อนไหว',     'ละเอียดอ่อน รู้สึกลึกซึ้ง',                      9);

-- ============================================================================
-- 3. ASSIGN tags to existing characters
-- ============================================================================

UPDATE characters SET appearance_tags = '{"cute","gentle_look"}',       personality_tags = '{"caring","sensitive"}'     WHERE name = 'แก้ว';
UPDATE characters SET appearance_tags = '{"cool","elegant"}',           personality_tags = '{"tsundere","mysterious"}'  WHERE name = 'คิระ';
UPDATE characters SET appearance_tags = '{"sporty","cute"}',            personality_tags = '{"cheerful","confident"}'   WHERE name = 'เคน';
UPDATE characters SET appearance_tags = '{"intellectual","cool"}',      personality_tags = '{"serious","caring"}'       WHERE name = 'ไคร์';
UPDATE characters SET appearance_tags = '{"elegant","wild"}',           personality_tags = '{"mysterious","flirty"}'    WHERE name = 'จันทร์';
UPDATE characters SET appearance_tags = '{"cool","intellectual"}',      personality_tags = '{"tsundere"}'               WHERE name = 'เจด';
UPDATE characters SET appearance_tags = '{"cool"}',                     personality_tags = '{"tsundere","serious"}'     WHERE name = 'เซน';
UPDATE characters SET appearance_tags = '{"gentle_look","intellectual"}', personality_tags = '{"mysterious","caring"}'  WHERE name = 'เซย์เมย์';
UPDATE characters SET appearance_tags = '{"cool","elegant"}',           personality_tags = '{"serious","mysterious"}'   WHERE name = 'แทน';
UPDATE characters SET appearance_tags = '{"sporty","cute"}',            personality_tags = '{"cheerful"}'               WHERE name = 'ธันวา';
UPDATE characters SET appearance_tags = '{"elegant"}',                  personality_tags = '{"flirty","confident"}'     WHERE name = 'ธาม';
UPDATE characters SET appearance_tags = '{"elegant","gentle_look"}',    personality_tags = '{"flirty","caring"}'        WHERE name = 'ธีร์';
UPDATE characters SET appearance_tags = '{"wild","elegant"}',           personality_tags = '{"mysterious","flirty"}'    WHERE name = 'นาเดีย';
UPDATE characters SET appearance_tags = '{"gentle_look","intellectual"}', personality_tags = '{"shy","sensitive"}'      WHERE name = 'นาวิน';
UPDATE characters SET appearance_tags = '{"cute","gentle_look"}',       personality_tags = '{"shy","sensitive"}'        WHERE name = 'นิ่ม';
UPDATE characters SET appearance_tags = '{"cool","intellectual"}',      personality_tags = '{"tsundere"}'               WHERE name = 'นิว';
UPDATE characters SET appearance_tags = '{"cute"}',                     personality_tags = '{"cheerful","serious"}'     WHERE name = 'พิม';
UPDATE characters SET appearance_tags = '{"gentle_look"}',              personality_tags = '{"caring","confident"}'     WHERE name = 'แพร';
UPDATE characters SET appearance_tags = '{"sporty"}',                   personality_tags = '{"cheerful","confident"}'   WHERE name = 'ฟ้าใส';
UPDATE characters SET appearance_tags = '{"cute","gentle_look"}',       personality_tags = '{"cheerful","caring"}'      WHERE name = 'เฟิร์น';
UPDATE characters SET appearance_tags = '{"gentle_look","cute"}',       personality_tags = '{"caring","cheerful"}'      WHERE name = 'ภูมิ';
UPDATE characters SET appearance_tags = '{"cute","elegant"}',           personality_tags = '{"flirty","confident"}'     WHERE name = 'มิน';
UPDATE characters SET appearance_tags = '{"cute"}',                     personality_tags = '{"shy","caring"}'           WHERE name = 'มินิ';
UPDATE characters SET appearance_tags = '{"cool","intellectual"}',      personality_tags = '{"serious","tsundere"}'     WHERE name = 'เมย์';
UPDATE characters SET appearance_tags = '{"elegant","cool"}',           personality_tags = '{"confident","sensitive"}'  WHERE name = 'โมเน่';
UPDATE characters SET appearance_tags = '{"gentle_look","intellectual"}', personality_tags = '{"mysterious","sensitive"}' WHERE name = 'ริน';
UPDATE characters SET appearance_tags = '{"cool","wild"}',              personality_tags = '{"confident","tsundere"}'   WHERE name = 'เรน';
UPDATE characters SET appearance_tags = '{"elegant","cute"}',           personality_tags = '{"confident","shy"}'        WHERE name = 'ลิลลี่';
UPDATE characters SET appearance_tags = '{"gentle_look","cute"}',       personality_tags = '{"shy","sensitive"}'        WHERE name = 'ลีโอ';
UPDATE characters SET appearance_tags = '{"wild","gentle_look"}',       personality_tags = '{"caring","sensitive"}'     WHERE name = 'วิน';
UPDATE characters SET appearance_tags = '{"elegant","sporty"}',         personality_tags = '{"confident","sensitive"}'  WHERE name = 'หยก';
UPDATE characters SET appearance_tags = '{"gentle_look"}',              personality_tags = '{"caring","shy"}'           WHERE name = 'หลิน';
UPDATE characters SET appearance_tags = '{"elegant","intellectual"}',   personality_tags = '{"caring","mysterious"}'    WHERE name = 'ออม';
UPDATE characters SET appearance_tags = '{"sporty"}',                   personality_tags = '{"cheerful","sensitive"}'   WHERE name = 'อาร์ม';
UPDATE characters SET appearance_tags = '{"cool","elegant"}',           personality_tags = '{"serious","tsundere"}'     WHERE name = 'ไอซ์';
UPDATE characters SET appearance_tags = '{"cool","intellectual"}',      personality_tags = '{"confident","tsundere"}'   WHERE name = 'ไอริน';
