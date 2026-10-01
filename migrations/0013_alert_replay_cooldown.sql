-- Preserve every terminal delivery while allowing an operator to create a new,
-- independently tracked attempt. Cooldown suppresses flapping notifications only;
-- it never hides incidents or monitored state.
ALTER TABLE alert_policies
    ADD COLUMN cooldown_seconds integer NOT NULL DEFAULT 0
        CHECK (cooldown_seconds BETWEEN 0 AND 86400);

ALTER TABLE alert_deliveries
    ADD COLUMN replay_of uuid,
    ADD FOREIGN KEY (tenant_id, replay_of)
        REFERENCES alert_deliveries(tenant_id, id) ON DELETE SET NULL (replay_of);

CREATE UNIQUE INDEX alert_deliveries_replay_idx
ON alert_deliveries (tenant_id, replay_of) WHERE replay_of IS NOT NULL;
