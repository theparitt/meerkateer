-- Credential use is tracked separately from agent last_seen_at so rotation and
-- compromise investigations can distinguish an agent identity from an individual key.
ALTER TABLE agent_credentials
ADD COLUMN last_used_at timestamptz;

CREATE INDEX agent_credentials_active_lookup_idx
ON agent_credentials (tenant_id, agent_id, prefix)
WHERE revoked_at IS NULL;
