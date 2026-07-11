-- token_version supports JWT revocation: bumped on password change, embedded in
-- issued JWTs, checked against the DB on every authenticated request.
ALTER TABLE users ADD COLUMN token_version INTEGER NOT NULL DEFAULT 0;

-- salt is dead weight: argon2's PHC string (password_hash) already embeds its own
-- salt, and this column was never read back anywhere.
ALTER TABLE users DROP COLUMN salt;
