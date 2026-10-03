\set ON_ERROR_STOP on

DO $$
BEGIN
    IF (SELECT count(*) FROM agents WHERE display_name LIKE 'Fleet Sim %' AND status = 'active') <> 10 THEN
        RAISE EXCEPTION 'expected ten active simulated fleet agents';
    END IF;
    IF (SELECT count(*) FROM project_agents pa JOIN agents a
          ON a.tenant_id = pa.tenant_id AND a.id = pa.agent_id
        WHERE a.display_name LIKE 'Fleet Sim %') <> 10 THEN
        RAISE EXCEPTION 'simulated fleet agents were not assigned to the workspace';
    END IF;
    IF (SELECT count(*) FROM agent_batches b JOIN agents a
          ON a.tenant_id = b.tenant_id AND a.id = b.agent_id
        WHERE a.display_name LIKE 'Fleet Sim %') <> 19 THEN
        RAISE EXCEPTION 'expected nineteen durable simulated fleet batches';
    END IF;
    IF (SELECT count(*) FROM agent_telemetry_records r JOIN agents a
          ON a.tenant_id = r.tenant_id AND a.id = r.agent_id
        WHERE a.display_name LIKE 'Fleet Sim %'
          AND r.name = 'agent.heartbeat'
          AND r.value = 1.0) <> 19 THEN
        RAISE EXCEPTION 'simulated fleet heartbeat records are incomplete';
    END IF;
    IF (SELECT max(s.last_sequence) FROM agent_sequence_state s JOIN agents a
          ON a.tenant_id = s.tenant_id AND a.id = s.agent_id
        WHERE a.display_name LIKE 'Fleet Sim %') <> 24 THEN
        RAISE EXCEPTION 'simulated fleet durable sequence did not advance through retry';
    END IF;
    IF (SELECT count(*) FROM outbox o JOIN agents a
          ON a.tenant_id = o.tenant_id
         AND a.id = (o.payload->>'agent_id')::uuid
        WHERE a.display_name LIKE 'Fleet Sim %'
          AND o.topic = 'agent.telemetry'
          AND o.processed_at IS NOT NULL
          AND o.dead_lettered_at IS NULL) <> 19 THEN
        RAISE EXCEPTION 'simulated fleet outbox records were not completed';
    END IF;
    IF EXISTS (
        SELECT 1 FROM agent_telemetry_records r JOIN agents a
          ON a.tenant_id = r.tenant_id AND a.id = r.agent_id
        WHERE a.display_name LIKE 'Fleet Sim %'
          AND (coalesce(r.message, '') ~ '(mka_agent_|mka_enroll_)'
               OR r.attributes::text ~ '(mka_agent_|mka_enroll_)')
    ) THEN
        RAISE EXCEPTION 'fleet credential-shaped data reached telemetry storage';
    END IF;
END;
$$;
