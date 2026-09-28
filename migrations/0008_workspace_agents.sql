-- A project is the durable workspace boundary inside a tenant/company. Agents remain
-- tenant-owned identities and can be assigned to more than one workspace when a shared
-- machine hosts workloads for multiple jobs.
ALTER TABLE enrollment_tokens
    ADD COLUMN project_id uuid;

ALTER TABLE enrollment_tokens
    ADD CONSTRAINT enrollment_tokens_project_fk
    FOREIGN KEY (tenant_id, project_id)
    REFERENCES projects(tenant_id, id)
    ON DELETE CASCADE;

CREATE TABLE project_agents (
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    agent_id uuid NOT NULL,
    assigned_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, project_id, agent_id),
    FOREIGN KEY (tenant_id, project_id)
        REFERENCES projects(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, agent_id)
        REFERENCES agents(tenant_id, id) ON DELETE CASCADE
);

CREATE INDEX project_agents_by_agent_idx
ON project_agents (tenant_id, agent_id, project_id);

ALTER TABLE project_agents ENABLE ROW LEVEL SECURITY;
ALTER TABLE project_agents FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON project_agents
    USING (tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = nullif(current_setting('meerkateer.tenant_id', true), '')::uuid);
