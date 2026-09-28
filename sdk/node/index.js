import { randomUUID } from "node:crypto";
import { isIP } from "node:net";

const environments = new Set(["development", "staging", "production", "test", "local"]);
const levels = new Set(["info", "warning", "error", "critical"]);
const statuses = new Set(["ok", "degraded", "down"]);
const deployStatuses = new Set(["started", "finished", "failed"]);
const identityPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;
const projectPattern = /^[a-z0-9][a-z0-9_-]{0,63}$/;
const kindPattern = /^[a-z0-9]+(_[a-z0-9]+)+$/;
const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const maxResponseBytes = 64 * 1024;

function endpointURL(value) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error("MEERKATEER_URL must be an absolute HTTP(S) URL");
  }
  if (!["http:", "https:"].includes(url.protocol) || !url.hostname || url.username || url.password || url.search || url.hash || url.pathname !== "/") {
    throw new Error("MEERKATEER_URL must be an origin without credentials or a path");
  }
  const host = url.hostname.replace(/^\[|\]$/g, "");
  const loopback = host === "localhost" || host === "127.0.0.1" || host === "::1" || (isIP(host) === 4 && host.startsWith("127."));
  if (url.protocol !== "https:" && !loopback) {
    throw new Error("HTTPS is required except for loopback development URLs");
  }
  return url;
}

function required(value, name, pattern) {
  if (typeof value !== "string" || !pattern.test(value)) throw new Error(`${name} is invalid`);
  return value;
}

function messageField(message) {
  if (message === undefined) return {};
  if (typeof message !== "string" || [...message].length > 1024) throw new Error("message is invalid");
  return { message };
}

export class Meerkateer {
  constructor({ url, serviceKey, project, service, environment = "production", timeoutMs = 5000, retries = 2, fetchImpl = fetch }) {
    this.url = endpointURL(url);
    this.serviceKey = required(serviceKey, "service key", /^mks_sk_[A-Za-z0-9_-]+$/);
    this.project = required(project, "project", projectPattern);
    this.service = required(service, "service", identityPattern);
    if (!environments.has(environment)) throw new Error("environment is invalid");
    if (!Number.isInteger(timeoutMs) || timeoutMs < 100 || timeoutMs > 60000) throw new Error("timeoutMs is invalid");
    if (!Number.isInteger(retries) || retries < 0 || retries > 5) throw new Error("retries is invalid");
    this.environment = environment;
    this.timeoutMs = timeoutMs;
    this.retries = retries;
    this.fetchImpl = fetchImpl;
  }

  static fromEnv(env = process.env) {
    return new Meerkateer({
      url: env.MEERKATEER_URL,
      serviceKey: env.MEERKATEER_SERVICE_KEY,
      project: env.MEERKATEER_PROJECT,
      service: env.MEERKATEER_SERVICE,
      environment: env.MEERKATEER_ENVIRONMENT || "production",
    });
  }

  heartbeat(status = "ok", { message, idempotencyKey } = {}) {
    if (!statuses.has(status)) throw new Error("heartbeat status is invalid");
    return this.send("heartbeat", { status, ...messageField(message) }, idempotencyKey);
  }

  event(kind, { level = "info", message, count = 1, idempotencyKey } = {}) {
    if (typeof kind !== "string" || kind.length > 128 || !kindPattern.test(kind)) throw new Error("event kind is invalid");
    if (!levels.has(level)) throw new Error("event level is invalid");
    if (!Number.isInteger(count) || count < 1 || count > 1_000_000) throw new Error("event count is invalid");
    return this.send("event", { kind, level, count, ...messageField(message) }, idempotencyKey);
  }

  deploy(version, commit, { status = "finished", idempotencyKey } = {}) {
    required(version, "version", /^.{1,128}$/);
    required(commit, "commit", /^.{1,128}$/);
    if (!deployStatuses.has(status)) throw new Error("deployment status is invalid");
    return this.send("deploy", { version, commit, status }, idempotencyKey);
  }

  async send(kind, fields, idempotencyKey = randomUUID()) {
    if (!uuidPattern.test(idempotencyKey)) throw new Error("idempotency key must be a UUID");
    const body = JSON.stringify({
      interface_version: "1",
      service: this.service,
      project: this.project,
      environment: this.environment,
      ...fields,
      timestamp: new Date().toISOString(),
    });
    if (Buffer.byteLength(body) > 16 * 1024) throw new Error("telemetry payload is too large");
    const destination = new URL(`v1/ingest/${kind}`, this.url);
    for (let attempt = 0; attempt <= this.retries; attempt++) {
      try {
        const response = await this.fetchImpl(destination, {
          method: "POST",
          redirect: "manual",
          signal: AbortSignal.timeout(this.timeoutMs),
          headers: {
            Authorization: `Bearer ${this.serviceKey}`,
            "Content-Type": "application/json",
            Accept: "application/json",
            "Idempotency-Key": idempotencyKey,
          },
          body,
        });
        if (response.status === 200 || response.status === 202) {
          const chunks = [];
          let size = 0;
          for await (const chunk of response.body) {
            size += chunk.byteLength;
            if (size > maxResponseBytes) {
              throw new Error("acknowledgement is too large");
            }
            chunks.push(chunk);
          }
          const acknowledgement = JSON.parse(Buffer.concat(chunks).toString("utf8"));
          if (acknowledgement?.idempotency_key !== idempotencyKey) throw new Error("acknowledgement key mismatch");
          return acknowledgement;
        }
        if (response.status !== 429 && response.status < 500) throw new Error(`Meerkateer rejected telemetry with HTTP ${response.status}`);
        if (attempt === this.retries) throw new Error("telemetry delivery failed after bounded retries");
      } catch (error) {
        if (attempt === this.retries || (error instanceof Error && error.message.startsWith("Meerkateer rejected"))) {
          throw new Error("telemetry delivery failed", { cause: error });
        }
      }
      await new Promise((resolve) => setTimeout(resolve, Math.min(250 * 2 ** attempt, 1000)));
    }
  }
}
