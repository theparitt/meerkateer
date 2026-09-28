import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

const health = {
  status: "ok",
  service: "meerkateer-server",
  project: "meerkateer",
  environment: "development",
  interface: "meerkateer",
  interface_version: "1",
  version: "0.1.0",
  timestamp: "2026-09-28T00:00:00Z",
  checks: { config: { ok: true } },
};

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

describe("App", () => {
  beforeEach(() => window.history.replaceState({}, "", "/app"));
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it("explains the public product and keeps bootstrap access off the landing page", () => {
    window.history.replaceState({}, "", "/");
    const fetch = vi.fn();
    vi.stubGlobal("fetch", fetch);
    render(<App />);
    expect(screen.getByRole("heading", { name: "Know when your servers need you." })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Open source. Self-hosted. Free." })).toBeTruthy();
    expect(
      screen.getByRole("heading", { name: "Hosted by us. No control-plane upkeep." }),
    ).toBeTruthy();
    expect(screen.queryByText("Admin token")).toBeNull();
    expect(fetch).not.toHaveBeenCalled();
  });

  it("opens self-hosted setup without loading the operations dashboard", () => {
    window.history.replaceState({}, "", "/setup");
    const fetch = vi.fn();
    vi.stubGlobal("fetch", fetch);
    const { container } = render(<App />);
    expect(screen.getByRole("heading", { name: "Make it yours." })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Create your company" })).toBeTruthy();
    expect(screen.getByRole("link", { name: /Read the self-hosting guide/ })).toBeTruthy();
    expect(container.querySelector(".status-grid")).toBeNull();
    expect(fetch).not.toHaveBeenCalled();
  });

  it("shows a clear path when setup was already completed", async () => {
    window.history.replaceState({}, "", "/setup");
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(json({ code: "bootstrap_already_completed" }, 409)),
    );
    const { container } = render(<App />);
    const form = container.querySelector("form");
    expect(form).toBeTruthy();
    const fields = form?.querySelectorAll("input");
    const values = [
      "setup-key",
      "Example Team",
      "example-team",
      "Owner",
      "owner@example.com",
      "long-password-value",
      "long-password-value",
    ];
    fields?.forEach((field, index) => {
      fireEvent.change(field, { target: { value: values[index] } });
    });
    if (form) fireEvent.submit(form);
    expect(await screen.findByText(/A company already exists here/)).toBeTruthy();
    expect(screen.getByRole("link", { name: "Set owner password →" }).getAttribute("href")).toBe(
      "/recover",
    );
  });

  it("signs in with owner email and password and explains invalid credentials", async () => {
    window.history.replaceState({}, "", "/login");
    const fetch = vi.fn().mockResolvedValue(json({ code: "invalid_credentials" }, 401));
    vi.stubGlobal("fetch", fetch);
    const { container } = render(<App />);
    const form = container.querySelector("form");
    expect(form).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Owner email"), {
      target: { value: "Owner@Example.com" },
    });
    fireEvent.change(screen.getByLabelText("Password"), {
      target: { value: "long-test-password" },
    });
    if (form) fireEvent.submit(form);
    expect(await screen.findByText(/The email or password is incorrect/)).toBeTruthy();
    expect(fetch.mock.calls[0][0]).toBe("/v1/session/password-login");
    expect(JSON.parse(fetch.mock.calls[0][1].body)).toMatchObject({
      email: "owner@example.com",
      password: "long-test-password",
    });
  });

  it("shows the clean login screen when the console has no session", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn((input: string | URL | Request) =>
        Promise.resolve(String(input) === "/health" ? json(health) : json({}, 401)),
      ),
    );
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Welcome back." })).toBeTruthy();
    expect(screen.getByLabelText("Owner email")).toBeTruthy();
    expect(screen.queryByText("Control plane")).toBeNull();
  });

  it("reports an unavailable API without leaking details", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("network unavailable")));
    render(<App />);
    expect(await screen.findByText("API unavailable")).toBeTruthy();
    expect(screen.getAllByRole("alert").length).toBeGreaterThan(0);
  });

  it("visualizes current state and durable outage causes", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn((input: string | URL | Request) => {
        const path = String(input);
        if (path === "/health") return Promise.resolve(json(health));
        if (path === "/v1/session") {
          return Promise.resolve(
            json({
              tenant_id: "00000000-0000-4000-8000-000000000001",
              user_id: "00000000-0000-4000-8000-000000000002",
              role: "owner",
              email: "owner@example.com",
              display_name: "Owner",
            }),
          );
        }
        if (path === "/v1/projects") {
          return Promise.resolve(
            json({
              items: [
                {
                  id: "00000000-0000-4000-8000-000000000003",
                  slug: "arena",
                  display_name: "Arena",
                  created_at: "2026-09-28T00:00:00Z",
                },
              ],
            }),
          );
        }
        if (path.endsWith("/agents")) {
          return Promise.resolve(
            json({
              items: [
                {
                  id: "00000000-0000-4000-8000-000000000006",
                  display_name: "Arena Host 01",
                  status: "active",
                  connection_state: "online",
                  enrolled_at: "2026-09-28T00:00:00Z",
                  last_seen_at: "2026-09-28T00:02:00Z",
                },
              ],
            }),
          );
        }
        if (path.includes("/timeline")) {
          return Promise.resolve(
            json({
              items: [
                {
                  idempotency_key: "00000000-0000-4000-8000-000000000005",
                  kind: "heartbeat",
                  state: "offline",
                  severity: null,
                  title: "Service reported offline",
                  message: "game process exited",
                  observed_at: "2026-09-28T00:02:00Z",
                  received_at: "2026-09-28T00:02:01Z",
                },
              ],
            }),
          );
        }
        return Promise.resolve(
          json({
            items: [
              {
                id: "00000000-0000-4000-8000-000000000004",
                project_id: "00000000-0000-4000-8000-000000000003",
                slug: "game-api",
                environment: "production",
                created_at: "2026-09-28T00:00:00Z",
                status: {
                  state: "offline",
                  reported_state: "offline",
                  stale: false,
                  last_sequence: 0,
                  observed_at: "2026-09-28T00:02:00Z",
                  updated_at: "2026-09-28T00:02:01Z",
                },
              },
            ],
          }),
        );
      }),
    );
    render(<App />);
    expect(await screen.findByText("game process exited")).toBeTruthy();
    expect(screen.getByText("Service reported offline")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Arena" })).toBeTruthy();
    expect(screen.getByText("Arena Host 01")).toBeTruthy();
    expect(screen.getByText("1 online")).toBeTruthy();
    expect(screen.getByText("Manage workspaces, machines, and processes")).toBeTruthy();
  });
});
