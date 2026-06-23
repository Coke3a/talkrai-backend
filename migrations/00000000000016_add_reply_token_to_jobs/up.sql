-- LINE webhook reply token, captured at job creation so the roleplay
-- response can be delivered via the free reply API when still within the
-- reply-token validity window (falls back to push otherwise).
-- Nullable: only message-event jobs carry a token; postback/stub jobs do not.
ALTER TABLE jobs ADD COLUMN reply_token VARCHAR;
