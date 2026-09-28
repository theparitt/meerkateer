CREATE TABLE system_state (
    singleton_id smallint PRIMARY KEY CHECK (singleton_id = 1),
    bootstrap_completed_at timestamptz,
    bootstrap_tenant_id uuid REFERENCES tenants(id),
    CHECK (
        (bootstrap_completed_at IS NULL AND bootstrap_tenant_id IS NULL)
        OR (bootstrap_completed_at IS NOT NULL AND bootstrap_tenant_id IS NOT NULL)
    )
);

INSERT INTO system_state (singleton_id) VALUES (1);

CREATE OR REPLACE FUNCTION prevent_bootstrap_reset() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.bootstrap_completed_at IS NOT NULL THEN
        RAISE EXCEPTION 'bootstrap state is irreversible';
    END IF;
    IF NEW.bootstrap_completed_at IS NULL OR NEW.bootstrap_tenant_id IS NULL THEN
        RAISE EXCEPTION 'bootstrap must be completed atomically';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER system_state_one_way_bootstrap
BEFORE UPDATE ON system_state
FOR EACH ROW EXECUTE FUNCTION prevent_bootstrap_reset();
