-- A destructive rollback would discard web identities and stories and restore incompatible NOT NULL columns.
DO $$ BEGIN RAISE EXCEPTION 'Forward-only migration: disable web admissions and deploy a compatible recovery release'; END $$;
