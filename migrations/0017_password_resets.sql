-- Email addresses are only kept so people can reset a forgotten password; invite
-- emails go out without the address being stored anywhere. Builds without the
-- `smtp` feature share the same tables and just never fill them.
ALTER TABLE users ADD COLUMN email TEXT;

CREATE TABLE password_resets (
    token_hash TEXT PRIMARY KEY,
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);
CREATE INDEX password_resets_user ON password_resets(user_id, created_at DESC);
