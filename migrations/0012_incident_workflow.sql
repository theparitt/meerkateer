-- Operator workflow is separate from monitored state. Acknowledging or assigning an
-- incident never resolves it; only fresh recovery evidence can do that.
ALTER TABLE incidents
    ADD COLUMN acknowledged_at timestamptz,
    ADD COLUMN acknowledged_by uuid,
    ADD COLUMN assigned_to uuid,
    ADD FOREIGN KEY (tenant_id, acknowledged_by)
        REFERENCES memberships(tenant_id, user_id) ON DELETE RESTRICT,
    ADD FOREIGN KEY (tenant_id, assigned_to)
        REFERENCES memberships(tenant_id, user_id) ON DELETE RESTRICT,
    ADD CHECK ((acknowledged_at IS NULL) = (acknowledged_by IS NULL));

CREATE TABLE incident_activity (
    tenant_id uuid NOT NULL,
    id uuid NOT NULL DEFAULT gen_random_uuid(),
    incident_id uuid NOT NULL,
    kind text NOT NULL CHECK (kind IN ('acknowledged', 'assigned', 'unassigned', 'note')),
    actor_id uuid NOT NULL,
    note text CHECK (note IS NULL OR char_length(note) BETWEEN 1 AND 2000),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, id),
    FOREIGN KEY (tenant_id, incident_id)
        REFERENCES incidents(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, actor_id)
        REFERENCES memberships(tenant_id, user_id) ON DELETE RESTRICT,
    CHECK ((kind = 'note') = (note IS NOT NULL))
);

CREATE INDEX incident_activity_timeline_idx
ON incident_activity (tenant_id, incident_id, created_at DESC, id);

ALTER TABLE incident_activity ENABLE ROW LEVEL SECURITY;
ALTER TABLE incident_activity FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON incident_activity
USING (tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid)
WITH CHECK (tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid);
