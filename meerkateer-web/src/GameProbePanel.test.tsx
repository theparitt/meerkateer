import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ServiceResponse } from "./api";
import { GameProbePanel } from "./GameProbePanel";

const service: ServiceResponse = {
  id: "00000000-0000-4000-8000-000000000004",
  project_id: "00000000-0000-4000-8000-000000000003",
  slug: "survival-01",
  environment: "production",
  created_at: "2026-09-28T00:00:00Z",
  game: { kind: "minecraft_java", host: "play.example.com", port: 25565 },
  status: {
    state: "unknown",
    reported_state: null,
    stale: false,
    last_sequence: null,
    observed_at: null,
    updated_at: null,
  },
};

describe("Minecraft game signals", () => {
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it("separates external response from process and game performance", async () => {
    Object.defineProperty(document, "cookie", {
      configurable: true,
      value: "meerkateer_csrf=test-proof",
    });
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          state: "responding",
          message: "Minecraft Java status answered from this probe.",
          probe_location: "community_control_plane",
          observed_at: "2026-09-28T00:10:00Z",
          response_ms: 42,
          version_name: "Paper 1.21",
          players_online: 0,
          players_max: 100,
        }),
        { status: 200 },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);
    render(<GameProbePanel service={service} />);
    expect(screen.getByText("Not linked to this instance")).toBeTruthy();
    expect(screen.getByText("Paper collector not connected")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Test game status now" }));
    expect(await screen.findByText("Responding from this probe")).toBeTruthy();
    expect(screen.getByText(/0\/100 players reported/)).toBeTruthy();
    expect(fetchMock.mock.calls[0][0]).toBe(`/v1/services/${service.id}/game-probe`);
    expect(fetchMock.mock.calls[0][1].headers).toMatchObject({ "X-Meerkateer-CSRF": "test-proof" });
  });
});
