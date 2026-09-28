ALTER TABLE users
    ADD COLUMN IF NOT EXISTS local_password_hash text
    CHECK (local_password_hash IS NULL OR local_password_hash LIKE '$argon2id$%');
