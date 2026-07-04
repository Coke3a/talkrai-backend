-- Reverses migration 022. Dropping the table also drops its indexes and the
-- RLS policy; the table-level GRANT disappears with it.
DROP TABLE IF EXISTS analytics_events;
