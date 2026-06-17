ALTER TABLE scenes
    ADD COLUMN suggested_first_replies TEXT[] NOT NULL DEFAULT '{}';
