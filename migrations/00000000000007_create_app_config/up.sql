CREATE TABLE app_config (
    key        VARCHAR PRIMARY KEY,
    value      VARCHAR NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO app_config (key, value) VALUES ('active_llm_provider', 'claude');
