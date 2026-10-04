import { type FormEvent, useState } from "react";
import {
  type NetworkProbeRequest,
  type NetworkProbeResponse,
  type ServiceResponse,
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

export function NetworkProbePanel({ service }: { service: ServiceResponse }) {
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

  return (
    <details className="network-probe-panel">
      <summary>
        <span aria-hidden="true">⌁</span>
        <span>
          <strong>Test a network destination</strong>
          <small>HTTP, HTTPS, TCP, DNS, or TLS · manual and not saved</small>
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
      </div>
    </details>
  );
}
