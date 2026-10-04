import { type FormEvent, useEffect, useState } from "react";
import {
  deleteScheduledProbe,
  fetchProbeObservations,
  fetchScheduledProbe,
  type NetworkProbeRequest,
  type NetworkProbeResponse,
  type ProbeObservationResponse,
  type ScheduledProbeResponse,
  type ScheduledProbeUpsertRequest,
  type ServiceResponse,
  saveScheduledProbe,
  testNetworkDestination,
} from "./api";
import "./network-probe.css";

type ProbeKind = NetworkProbeRequest["kind"];

const defaultPorts: Record<ProbeKind, string> = {
  http: "80",
  https: "443",
  tcp: "",
  dns: "",
  tls: "443",
};

function resultTone(state: NetworkProbeResponse["state"]): string {
  if (state === "responding") return "network-result-ok";
  if (state === "unexpected_status") return "network-result-warn";
  return "network-result-error";
}

export function NetworkProbePanel({
  service,
  canManage,
}: {
  service: ServiceResponse;
  canManage: boolean;
}) {
  const initialHost = service.game?.host ?? "";
  const initialPort = service.game?.port ? String(service.game.port) : "443";
  const [kind, setKind] = useState<ProbeKind>(service.game ? "tcp" : "https");
  const [host, setHost] = useState(initialHost);
  const [port, setPort] = useState(service.game ? initialPort : "443");
  const [path, setPath] = useState("/");
  const [expectedStatus, setExpectedStatus] = useState("");
  const [testing, setTesting] = useState(false);
  const [result, setResult] = useState<NetworkProbeResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [scheduled, setScheduled] = useState<ScheduledProbeResponse | null>(null);
  const [history, setHistory] = useState<ProbeObservationResponse[]>([]);
  const [intervalSeconds, setIntervalSeconds] = useState("15");
  const [failureThreshold, setFailureThreshold] = useState("2");
  const [recoveryThreshold, setRecoveryThreshold] = useState("1");
  const [scheduleLoading, setScheduleLoading] = useState(true);
  const [scheduleSaving, setScheduleSaving] = useState(false);
  const [scheduleError, setScheduleError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    setScheduleLoading(true);
    Promise.all([
      fetchScheduledProbe(service.id, controller.signal),
      fetchProbeObservations(service.id, controller.signal),
    ])
      .then(([probe, observations]) => {
        setScheduled(probe);
        setHistory(observations);
        if (probe) {
          setKind(probe.kind);
          setHost(probe.host);
          setPort(probe.port === null ? "" : String(probe.port));
          setPath(probe.path ?? "/");
          setExpectedStatus(probe.expected_status === null ? "" : String(probe.expected_status));
          setIntervalSeconds(String(probe.interval_seconds));
          setFailureThreshold(String(probe.failure_threshold));
          setRecoveryThreshold(String(probe.recovery_threshold));
        }
      })
      .catch((failure: unknown) => {
        if (!controller.signal.aborted) {
          setScheduleError(
            failure instanceof Error ? failure.message : "Scheduled monitor could not be loaded",
          );
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setScheduleLoading(false);
      });
    return () => controller.abort();
  }, [service.id]);

  function changeKind(next: ProbeKind) {
    setKind(next);
    setPort(defaultPorts[next]);
    setResult(null);
    setError(null);
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setTesting(true);
    setError(null);
    setResult(null);
    const parsedPort = port === "" ? null : Number(port);
    const parsedExpected = expectedStatus === "" ? null : Number(expectedStatus);
    const request: NetworkProbeRequest = {
      kind,
      host: host.trim(),
      port: parsedPort,
      path: ["http", "https", "tls"].includes(kind) ? path : null,
      expected_status: ["http", "https"].includes(kind) ? parsedExpected : null,
      timeout_ms: 5_000,
    };
    try {
      setResult(await testNetworkDestination(service.id, request));
    } catch (failure) {
      const detail = failure instanceof Error ? failure.message : "Network check failed";
      setError(
        detail.includes("unsafe_probe_destination")
          ? "This name resolves to a private, local, reserved, or mixed-trust address. The control plane blocks it to protect your network."
          : detail.includes("invalid_probe_request")
            ? "Check the host, port, path, and expected status. Enter a bare hostname without http:// or credentials."
            : detail,
      );
    } finally {
      setTesting(false);
    }
  }

  function scheduleRequest(enabled: boolean): ScheduledProbeUpsertRequest {
    return {
      kind,
      host: host.trim(),
      port: port === "" ? null : Number(port),
      path: ["http", "https", "tls"].includes(kind) ? path : null,
      expected_status: ["http", "https"].includes(kind)
        ? expectedStatus === ""
          ? null
          : Number(expectedStatus)
        : null,
      timeout_ms: 5_000,
      interval_seconds: Number(intervalSeconds),
      failure_threshold: Number(failureThreshold),
      recovery_threshold: Number(recoveryThreshold),
      enabled,
    };
  }

  async function saveMonitor(enabled = scheduled?.enabled ?? true) {
    setScheduleSaving(true);
    setScheduleError(null);
    try {
      setScheduled(await saveScheduledProbe(service.id, scheduleRequest(enabled)));
    } catch (failure) {
      const detail = failure instanceof Error ? failure.message : "Scheduled monitor failed";
      setScheduleError(
        detail.includes("service_has_active_sdk_credential")
          ? "This service already receives SDK heartbeats. Create a separate service for an external scheduled monitor so two health sources cannot contradict each other."
          : detail.includes("invalid_probe_request")
            ? "Check the destination and schedule values before saving."
            : detail,
      );
    } finally {
      setScheduleSaving(false);
    }
  }

  async function removeMonitor() {
    if (!window.confirm("Delete this scheduled monitor and its bounded check history?")) return;
    setScheduleSaving(true);
    setScheduleError(null);
    try {
      await deleteScheduledProbe(service.id);
      setScheduled(null);
      setHistory([]);
    } catch (failure) {
      const detail = failure instanceof Error ? failure.message : "Scheduled monitor failed";
      setScheduleError(
        detail.includes("probe_must_recover_before_delete")
          ? "This monitor is still offline. Let it recover before deletion so its active incident is not hidden."
          : detail,
      );
    } finally {
      setScheduleSaving(false);
    }
  }

  async function refreshMonitor() {
    const controller = new AbortController();
    setScheduleLoading(true);
    setScheduleError(null);
    try {
      const [probe, observations] = await Promise.all([
        fetchScheduledProbe(service.id, controller.signal),
        fetchProbeObservations(service.id, controller.signal),
      ]);
      setScheduled(probe);
      setHistory(observations);
    } catch (failure) {
      setScheduleError(
        failure instanceof Error ? failure.message : "Scheduled monitor could not be refreshed",
      );
    } finally {
      setScheduleLoading(false);
    }
  }

  return (
    <details className="network-probe-panel">
      <summary>
        <span aria-hidden="true">⌁</span>
        <span>
          <strong>Test a network destination</strong>
          <small>HTTP, HTTPS, TCP, DNS, or TLS · one-off or scheduled</small>
        </span>
      </summary>
      <div className="network-probe-body">
        <p>
          Run one check from this Community control plane. Private and reserved addresses are
          blocked, every DNS answer is checked, and redirects are never followed.
        </p>
        <form className="network-probe-form" onSubmit={(event) => void submit(event)}>
          <label>
            <span>Check</span>
            <select
              aria-label="Network check type"
              value={kind}
              onChange={(event) => changeKind(event.target.value as ProbeKind)}
            >
              <option value="https">HTTPS</option>
              <option value="http">HTTP</option>
              <option value="tcp">TCP port</option>
              <option value="dns">DNS</option>
              <option value="tls">TLS certificate</option>
            </select>
          </label>
          <label className="network-host-field">
            <span>Public hostname or IP</span>
            <input
              required
              maxLength={253}
              placeholder="status.example.com"
              value={host}
              onChange={(event) => setHost(event.target.value)}
            />
          </label>
          {kind !== "dns" ? (
            <label>
              <span>Port</span>
              <input
                required={kind === "tcp"}
                inputMode="numeric"
                min={1}
                max={65_535}
                type="number"
                value={port}
                onChange={(event) => setPort(event.target.value)}
              />
            </label>
          ) : null}
          {["http", "https", "tls"].includes(kind) ? (
            <label className="network-path-field">
              <span>Path</span>
              <input
                required
                maxLength={2_048}
                value={path}
                onChange={(event) => setPath(event.target.value)}
              />
            </label>
          ) : null}
          {["http", "https"].includes(kind) ? (
            <label>
              <span>Expected status</span>
              <input
                inputMode="numeric"
                min={100}
                max={599}
                placeholder="2xx / 3xx"
                type="number"
                value={expectedStatus}
                onChange={(event) => setExpectedStatus(event.target.value)}
              />
            </label>
          ) : null}
          <button disabled={testing} type="submit">
            {testing ? "Checking…" : "Run check"}
          </button>
        </form>
        {result ? (
          <article className={`network-probe-result ${resultTone(result.state)}`} role="status">
            <span className="network-result-dot" aria-hidden="true" />
            <div>
              <strong>{result.state.replaceAll("_", " ")}</strong>
              <p>{result.message}</p>
              <small>
                {result.response_ms === null ? "Time unavailable" : `${result.response_ms} ms`}
                {result.status_code === null ? "" : ` · HTTP ${result.status_code}`}
                {` · ${result.resolved_addresses} public DNS answer${result.resolved_addresses === 1 ? "" : "s"}`}
                {result.tls_days_remaining === null
                  ? ""
                  : ` · certificate ${result.tls_days_remaining} days remaining`}
              </small>
            </div>
          </article>
        ) : null}
        {error ? (
          <p className="network-probe-error" role="alert">
            {error}
          </p>
        ) : null}
        <section className="network-schedule" aria-label="Scheduled network monitor">
          <div className="network-schedule-head">
            <div>
              <p className="eyebrow">Scheduled monitor</p>
              <h4>{scheduled ? "Watching automatically" : "Turn this check into a monitor"}</h4>
              <p>
                Two failures are required by default before opening an incident. One fresh success
                recovers it. This service then uses the probe—not an SDK key—as its health source.
              </p>
            </div>
            {scheduled ? (
              <span className={`network-consensus consensus-${scheduled.consensus_state}`}>
                {scheduled.enabled ? scheduled.consensus_state : "paused"}
              </span>
            ) : null}
          </div>
          <div className="network-schedule-settings">
            <label>
              <span>Every</span>
              <select
                aria-label="Scheduled check interval"
                value={intervalSeconds}
                onChange={(event) => setIntervalSeconds(event.target.value)}
                disabled={!canManage || scheduleSaving}
              >
                <option value="15">15 seconds</option>
                <option value="30">30 seconds</option>
                <option value="60">1 minute</option>
                <option value="300">5 minutes</option>
              </select>
            </label>
            <label>
              <span>Fail after</span>
              <select
                aria-label="Failure confirmation count"
                value={failureThreshold}
                onChange={(event) => setFailureThreshold(event.target.value)}
                disabled={!canManage || scheduleSaving}
              >
                <option value="1">1 failed check</option>
                <option value="2">2 failed checks</option>
                <option value="3">3 failed checks</option>
              </select>
            </label>
            <label>
              <span>Recover after</span>
              <select
                aria-label="Recovery confirmation count"
                value={recoveryThreshold}
                onChange={(event) => setRecoveryThreshold(event.target.value)}
                disabled={!canManage || scheduleSaving}
              >
                <option value="1">1 good check</option>
                <option value="2">2 good checks</option>
                <option value="3">3 good checks</option>
              </select>
            </label>
          </div>
          {scheduled ? (
            <div className="network-schedule-facts">
              <span>
                <small>Last result</small>
                <strong>{scheduled.last_state?.replaceAll("_", " ") ?? "Waiting"}</strong>
              </span>
              <span>
                <small>Confirmed failures</small>
                <strong>
                  {scheduled.consecutive_failures}/{scheduled.failure_threshold}
                </strong>
              </span>
              <span>
                <small>Next check</small>
                <strong>
                  {scheduled.enabled
                    ? new Intl.DateTimeFormat(undefined, {
                        hour: "numeric",
                        minute: "2-digit",
                        second: "2-digit",
                      }).format(new Date(scheduled.next_run_at))
                    : "Paused"}
                </strong>
              </span>
            </div>
          ) : null}
          {canManage ? (
            <div className="network-schedule-actions">
              <button
                className="schedule-primary"
                disabled={scheduleLoading || scheduleSaving || !host.trim()}
                onClick={() => void saveMonitor(scheduled?.enabled ?? true)}
                type="button"
              >
                {scheduleSaving ? "Saving…" : scheduled ? "Save settings" : "Start monitoring"}
              </button>
              {scheduled ? (
                <>
                  <button
                    disabled={scheduleSaving}
                    onClick={() => void saveMonitor(!scheduled.enabled)}
                    type="button"
                  >
                    {scheduled.enabled ? "Pause" : "Resume"}
                  </button>
                  <button
                    className="schedule-danger"
                    disabled={scheduleSaving}
                    onClick={() => void removeMonitor()}
                    type="button"
                  >
                    Delete
                  </button>
                </>
              ) : null}
            </div>
          ) : (
            <p className="network-viewer-note">
              Viewer access can inspect results but not edit them.
            </p>
          )}
          {scheduled ? (
            <div className="network-history">
              <div className="network-history-head">
                <strong>Recent checks</strong>
                <button
                  disabled={scheduleLoading}
                  onClick={() => void refreshMonitor()}
                  type="button"
                >
                  {scheduleLoading ? "Refreshing…" : "Refresh"}
                </button>
              </div>
              {history.length === 0 ? (
                <p>Waiting for the first scheduled check.</p>
              ) : (
                <ol>
                  {history.slice(0, 8).map((observation) => (
                    <li key={observation.id}>
                      <span
                        className={`history-dot ${observation.state === "responding" ? "history-ok" : "history-failed"}`}
                        aria-hidden="true"
                      />
                      <span>
                        <strong>{observation.state.replaceAll("_", " ")}</strong>
                        <small>{observation.message}</small>
                      </span>
                      <time dateTime={observation.observed_at}>
                        {new Intl.DateTimeFormat(undefined, {
                          hour: "numeric",
                          minute: "2-digit",
                          second: "2-digit",
                        }).format(new Date(observation.observed_at))}
                      </time>
                    </li>
                  ))}
                </ol>
              )}
            </div>
          ) : null}
          {scheduleError ? (
            <p className="network-probe-error" role="alert">
              {scheduleError}
            </p>
          ) : null}
        </section>
      </div>
    </details>
  );
}
