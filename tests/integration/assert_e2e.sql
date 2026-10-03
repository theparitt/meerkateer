\set ON_ERROR_STOP on

DO $$
BEGIN
    IF (SELECT count(*) FROM ingest_messages) <> 13 THEN
        RAISE EXCEPTION 'expected 13 durable MKS messages';
    END IF;
    IF (SELECT count(*) FROM service_snapshots WHERE state = 'online') <> 1 THEN
        RAISE EXCEPTION 'expected one online service snapshot';
    END IF;
    IF (SELECT count(*) FROM service_credentials) <> 2
       OR (SELECT count(*) FROM service_credentials WHERE password_hash LIKE '$argon2id$%') <> 2
       OR (SELECT count(*) FROM service_credentials WHERE last_used_at IS NOT NULL) <> 2 THEN
        RAISE EXCEPTION 'service credential hash/use evidence is incomplete';
    END IF;
    IF (SELECT count(*) FROM agent_batches) <> 4
       OR (SELECT count(*) FROM agent_batches WHERE gap_detected) <> 1 THEN
        RAISE EXCEPTION 'agent batch or gap evidence is incorrect';
    END IF;
    IF (SELECT count(*) FROM agent_telemetry_records) <> 13
       OR (SELECT count(*) FROM agent_telemetry_records
             WHERE name IN ('host.disk.inodes_used', 'host.disk.inodes_total')) <> 2
       OR (SELECT max(last_sequence) FROM agent_sequence_state) <> 44 THEN
        RAISE EXCEPTION 'agent record or sequence projection is incorrect';
    END IF;
    IF (SELECT count(*) FROM agent_credentials) <> 3
       OR (SELECT count(*) FROM agent_credentials WHERE password_hash LIKE '$argon2id$%') <> 3
       OR (SELECT count(*) FROM agent_credentials WHERE revoked_at IS NOT NULL) <> 3
       OR (SELECT count(DISTINCT id) FROM agent_credentials) <> 3 THEN
        RAISE EXCEPTION 'agent credential hash/revocation evidence is incomplete';
    END IF;
    IF (SELECT count(*) FROM agents WHERE status = 'revoked') <> 1 THEN
        RAISE EXCEPTION 'agent was not revoked';
    END IF;
    IF (SELECT count(*) FROM project_agents) <> 1 THEN
        RAISE EXCEPTION 'workspace machine assignment is incorrect';
    END IF;
    IF (SELECT count(*) FROM outbox WHERE topic LIKE 'ingest.%') <> 13
       OR (SELECT count(*) FROM outbox WHERE topic = 'agent.telemetry') <> 4 THEN
        RAISE EXCEPTION 'transactional outbox evidence is incomplete';
    END IF;
    IF (SELECT count(*) FROM outbox
        WHERE (topic LIKE 'ingest.%' OR topic = 'agent.telemetry')
          AND processed_at IS NOT NULL
          AND dead_lettered_at IS NULL) <> 17 THEN
        RAISE EXCEPTION 'supported outbox records were not completed';
    END IF;
    IF (SELECT count(*) FROM worker_runtime) <> 1
       OR (SELECT count(*) FROM worker_runtime
           WHERE last_cycle_at >= now() - interval '30 seconds'
             AND claimed = completed + retried + dead_lettered) <> 1 THEN
        RAISE EXCEPTION 'durable worker progress evidence is missing or inconsistent';
    END IF;
    IF (SELECT count(*) FROM incidents) <> 2
       OR (SELECT count(*) FROM incidents WHERE status = 'resolved') <> 2
       OR (SELECT count(*) FROM incidents WHERE cause = 'game process exited') <> 1
       OR (SELECT count(*) FROM incidents WHERE cause = 'planned process restart') <> 1
       OR (SELECT count(*) FROM incidents WHERE acknowledged_at IS NOT NULL) <> 1
       OR (SELECT count(*) FROM incidents WHERE assigned_to IS NOT NULL) <> 0 THEN
        RAISE EXCEPTION 'incident correlation or recovery evidence is incorrect';
    END IF;
    IF (SELECT count(*) FROM incident_activity) <> 4
       OR (SELECT count(*) FROM incident_activity WHERE kind = 'acknowledged') <> 1
       OR (SELECT count(*) FROM incident_activity WHERE kind = 'assigned') <> 1
       OR (SELECT count(*) FROM incident_activity WHERE kind = 'unassigned') <> 1
       OR (SELECT count(*) FROM incident_activity WHERE kind = 'note'
           AND note = 'ตรวจสอบแล้ว: process หยุดจริง') <> 1 THEN
        RAISE EXCEPTION 'incident operator activity is incomplete or duplicated';
    END IF;
    IF (SELECT count(*) FROM alert_deliveries) <> 4
       OR (SELECT count(*) FROM alert_deliveries WHERE status = 'skipped_unconfigured') <> 2
       OR (SELECT count(*) FROM alert_deliveries WHERE status = 'suppressed') <> 2 THEN
        RAISE EXCEPTION 'alert outcome history is incomplete';
    END IF;
    IF (SELECT count(*) FROM maintenance_windows WHERE cancelled_at IS NOT NULL) <> 1
       OR (SELECT count(*) FROM alert_policies WHERE enabled = false AND cooldown_seconds = 120) <> 1 THEN
        RAISE EXCEPTION 'maintenance or alert policy state is incorrect';
    END IF;
    IF (SELECT count(*) FROM outbox
        WHERE topic = 'test.retry'
          AND processed_at IS NULL
          AND attempts = 1
          AND available_at > created_at
          AND locked_at IS NULL
          AND locked_by IS NULL
          AND last_error = 'unsupported outbox topic') <> 1 THEN
        RAISE EXCEPTION 'failed outbox record was not safely scheduled for retry';
    END IF;
    IF (SELECT count(*) FROM outbox
        WHERE topic = 'test.dead-letter'
          AND processed_at IS NOT NULL
          AND dead_lettered_at IS NOT NULL
          AND attempts = 5) <> 1
       OR (SELECT count(*) FROM outbox_dead_letters
           WHERE topic = 'test.dead-letter'
             AND attempts = 5
             AND last_error = 'unsupported outbox topic') <> 1 THEN
        RAISE EXCEPTION 'poison outbox record did not reach the dead-letter queue';
    END IF;
    IF EXISTS (
        SELECT 1 FROM ingest_messages
        WHERE payload::text ~ '(mks_sk_|mka_agent_|mka_enroll_)'
    ) OR EXISTS (
        SELECT 1 FROM agent_telemetry_records
        WHERE coalesce(message, '') ~ '(mks_sk_|mka_agent_|mka_enroll_)'
           OR attributes::text ~ '(mks_sk_|mka_agent_|mka_enroll_)'
    ) THEN
        RAISE EXCEPTION 'credential-shaped data reached telemetry storage';
    END IF;
    IF (SELECT count(*) FROM audit_events WHERE action = 'tenant.bootstrap') <> 1
       OR (SELECT count(*) FROM audit_events WHERE action = 'service_credential.rotate') <> 1
       OR (SELECT count(*) FROM audit_events WHERE action = 'agent.enroll') <> 1
       OR (SELECT count(*) FROM audit_events WHERE action = 'workspace.agent.assign') <> 1
       OR (SELECT count(*) FROM audit_events WHERE action = 'workspace.agent.unassign') <> 1
       OR (SELECT count(*) FROM audit_events WHERE action = 'agent_credential.rotate') <> 2
       OR (SELECT count(*) FROM audit_events WHERE action = 'agent.revoke') <> 1 THEN
        RAISE EXCEPTION 'credential lifecycle audit evidence is incomplete';
    END IF;
END;
$$;
