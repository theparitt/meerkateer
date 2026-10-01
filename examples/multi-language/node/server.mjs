import http from "node:http";
import { Meerkateer } from "../../../sdk/node/index.js";

const language = "node";
const port = Number.parseInt(process.env.DEMO_PORT || "19102", 10);
const interval = Number.parseInt(process.env.DEMO_HEARTBEAT_SECONDS || "5", 10) * 1000;
const client = Meerkateer.fromEnv();
let state = "ok";

async function signal(nextState) {
  const messages = {
    ok: "demo service recovered",
    degraded: "demo dependency is slow",
    down: "demo process is unavailable",
  };
  const previous = state;
  state = nextState;
  await client.heartbeat(nextState, { message: messages[nextState] });
  if (previous !== nextState) {
    const level = { ok: "info", degraded: "warning", down: "error" }[nextState];
    const kind = { ok: "demo_recovered", degraded: "demo_degraded", down: "demo_failed" }[nextState];
    await client.event(kind, { level, message: messages[nextState] });
  }
}

function reply(response, statusCode, payload) {
  const body = JSON.stringify(payload);
  response.writeHead(statusCode, {
    "Content-Type": "application/json",
    "Content-Length": Buffer.byteLength(body),
  });
  response.end(body);
}

const server = http.createServer(async (request, response) => {
  if (request.method === "GET" && request.url === "/health") {
    reply(response, 200, { language, status: state });
    return;
  }
  const match = request.method === "POST" && request.url?.match(/^\/scenario\/(healthy|degraded|down)$/);
  if (!match) {
    reply(response, 404, { error: "not_found" });
    return;
  }
  const nextState = match[1] === "healthy" ? "ok" : match[1];
  try {
    await signal(nextState);
    reply(response, 200, { language, status: nextState });
  } catch (error) {
    reply(response, 502, { error: "telemetry_failed", detail: error instanceof Error ? error.message : "unknown" });
  }
});

await client.deploy("demo-1", "local", { status: "finished" });
await signal("ok");
setInterval(() => {
  client.heartbeat(state, { message: `${language} demo heartbeat` }).catch((error) => {
    console.error(`heartbeat delivery failed: ${error.message}`);
  });
}, interval).unref();
server.listen(port, "127.0.0.1", () => {
  console.log(`${language} demo listening on http://127.0.0.1:${port}`);
});
