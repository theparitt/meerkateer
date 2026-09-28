import { useState } from "react";
import { type GameProbeResponse, type ServiceResponse, testMinecraftStatus } from "./api";
import "./game-probe.css";

export function GameProbePanel({ service }: { service: ServiceResponse }) {
  const [result, setResult] = useState<GameProbeResponse | null>(null);
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (service.game?.kind !== "minecraft_java") return null;

  async function runTest() {
    setTesting(true);
    setError(null);
    try {
      setResult(await testMinecraftStatus(service.id));
    } catch (failure) {
      const detail = failure instanceof Error ? failure.message : "Status test failed";
      setError(
        detail.includes("unsafe_probe_destination")
          ? "This address resolves to a private or reserved destination. Public probe tests are blocked for safety."
          : detail,
      );
    } finally {
      setTesting(false);
    }
  }

  return (
    <section className="game-probe-panel" aria-label="Minecraft game signals">
      <div className="game-probe-head">
        <div>
          <p className="eyebrow">Minecraft Java / Paper</p>
          <h3>What can we actually see?</h3>
          <p>
            {service.game.host}:{service.game.port} · one game instance
          </p>
        </div>
        <button disabled={testing} onClick={() => void runTest()} type="button">
          {testing ? "Checking…" : "Test game status now"}
        </button>
      </div>
      <div className="game-signal-grid">
        <article
          className={`game-signal ${result?.state === "responding" ? "signal-ok" : result ? "signal-warn" : ""}`}
        >
          <span aria-hidden="true">◉</span>
          <h4>External game response</h4>
          <strong>
            {result?.state === "responding"
              ? "Responding from this probe"
              : result
                ? result.state.replaceAll("_", " ")
                : "Not tested yet"}
          </strong>
          <p>{result?.message ?? "Run a manual Minecraft Java status query."}</p>
          {result?.state === "responding" ? (
            <small>
              {result.response_ms === null
                ? "Response time unavailable"
                : `${result.response_ms} ms from this probe`}
              {result.players_online === null || result.players_max === null
                ? " · player count not reported"
                : ` · ${result.players_online}/${result.players_max} players reported`}
              {result.version_name ? ` · ${result.version_name}` : ""}
            </small>
          ) : null}
        </article>
        <article className="game-signal">
          <span aria-hidden="true">▣</span>
          <h4>Host / process</h4>
          <strong>Not linked to this instance</strong>
          <p>
            A machine can host several games. Host status alone does not prove this game responds.
          </p>
        </article>
        <article className="game-signal">
          <span aria-hidden="true">✦</span>
          <h4>Game performance</h4>
          <strong>Paper collector not connected</strong>
          <p>TPS and MSPT need a scoped collector. Missing metrics are never shown as zero.</p>
        </article>
      </div>
      {result ? (
        <p className="game-probe-caption" role="status">
          Checked{" "}
          {new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(
            new Date(result.observed_at),
          )}{" "}
          from {result.probe_location.replaceAll("_", " ")}. This manual result is not retained or
          used for alerts yet.
        </p>
      ) : null}
      {error ? (
        <p className="game-probe-error" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  );
}
