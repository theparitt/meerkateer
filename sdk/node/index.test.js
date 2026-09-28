import assert from "node:assert/strict";
import { test } from "node:test";
import { Meerkateer } from "./index.js";

const config = {
  url: "http://127.0.0.1:6510",
  serviceKey: "mks_sk_fixture123",
  project: "store",
  service: "worker-01",
  environment: "test",
};

test("sends a valid heartbeat and retains its ID across a retry", async () => {
  const calls = [];
  const watch = new Meerkateer({
    ...config,
    retries: 1,
    fetchImpl: async (url, options) => {
      calls.push({ url: String(url), options });
      if (calls.length === 1) return new Response("busy", { status: 503 });
      return Response.json({ status: "accepted", idempotency_key: options.headers["Idempotency-Key"] }, { status: 202 });
    },
  });
  await watch.heartbeat("ok", { message: "worker ready" });
  assert.equal(calls.length, 2);
  assert.equal(calls[0].options.headers["Idempotency-Key"], calls[1].options.headers["Idempotency-Key"]);
  assert.equal(calls[0].options.body, calls[1].options.body);
  assert.equal(calls[0].options.redirect, "manual");
  assert.equal(calls[0].url, "http://127.0.0.1:6510/v1/ingest/heartbeat");
  assert.equal(JSON.parse(calls[0].options.body).status, "ok");
});

test("rejects insecure destinations, redirects, and invalid event kinds", async () => {
  assert.throws(() => new Meerkateer({ ...config, url: "http://example.com" }), /HTTPS/);
  const watch = new Meerkateer({ ...config, retries: 0, fetchImpl: async () => new Response(null, { status: 302 }) });
  assert.throws(() => watch.event("Bad kind"), /event kind/);
  await assert.rejects(watch.heartbeat("ok"), /telemetry delivery failed/);
});
