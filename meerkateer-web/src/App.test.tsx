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
  version: "0.2.0",
  timestamp: "2026-09-28T00:00:00Z",
  checks: { config: { ok: true } },
};

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function emptyConsoleFetch(role: "owner" | "viewer" = "owner") {
  return vi.fn((input: string | URL | Request) => {
    const path = String(input);
    if (path === "/health") return Promise.resolve(json(health));
    if (path === "/v1/session") {
      return Promise.resolve(
        json({
          tenant_id: "00000000-0000-4000-8000-000000000001",
          user_id: "00000000-0000-4000-8000-000000000002",
          role,
          email: `${role}@example.com`,
          display_name: role === "owner" ? "Owner" : "Read-only Operator",
        }),
      );
    }
    if (path === "/v1/alerts/policy") {
      return Promise.resolve(
        json({
          enabled: true,
          notify_down: true,
          notify_recovered: true,
          cooldown_seconds: 0,
          webhook_configured: false,
          updated_at: null,
        }),
      );
    }
    if (path === "/v1/admin/summary") {
      return Promise.resolve(
        json({
          tenant_id: "00000000-0000-4000-8000-000000000001",
          deployment_mode: "community",
          projects: 0,
          services: 0,
          agents: 0,
          open_incidents: 0,
          pending_alerts: 0,
          dead_lettered_alerts: 0,
          active_maintenance_windows: 0,
          oldest_pending_alert_at: null,
          worker_status: "never_seen",
          worker_started_at: null,
          worker_last_cycle_at: null,
          worker_last_cycle_claimed: 0,
          worker_last_cycle_completed: 0,
          worker_last_cycle_retried: 0,
          worker_last_cycle_dead_lettered: 0,
        }),
      );
    }
    return Promise.resolve(json({ items: [] }));
  });
}

describe("App", () => {
  beforeEach(() => window.history.replaceState({}, "", "/app"));
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    // biome-ignore lint/suspicious/noDocumentCookie: jsdom needs the same cookie-backed CSRF path as the browser.
    document.cookie = "meerkateer_csrf=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/";
    Reflect.deleteProperty(navigator, "clipboard");
  });

  it("explains the public product and keeps bootstrap access off the landing page", () => {
    window.history.replaceState({}, "", "/");
    const fetch = vi.fn();
    vi.stubGlobal("fetch", fetch);
    const { container } = render(<App />);
    expect(
      screen.getByRole("heading", { name: "Know what broke, and when it recovered." }),
    ).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Open source. Self-hosted. Free." })).toBeTruthy();
    expect(
      screen.getByRole("heading", { name: "Hosted by us. Built from public core." }),
    ).toBeTruthy();
    expect(screen.getByText("v0.2.0")).toBeTruthy();
    expect(screen.getByText("Current phase: Community Alpha")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Install one small Controller." })).toBeTruthy();
    expect(container.querySelectorAll(".controller-download-card")).toHaveLength(2);
    expect(screen.getByRole("img", { name: "Windows" })).toBeTruthy();
    expect(screen.getByRole("img", { name: "Ubuntu Linux" })).toBeTruthy();
    expect(
      screen.getByText(/portable build intentionally does not modify Windows startup/),
    ).toBeTruthy();
    const windowsMsiLink = screen.getByRole("link", { name: "Download MSI" });
    const windowsCliLink = screen.getByRole("link", { name: "Download CLI ZIP" });
    expect(windowsMsiLink.closest("article")).toBe(windowsCliLink.closest("article"));
    expect(windowsMsiLink.getAttribute("href")).toContain(
      "meerkateer-controller-windows-x86_64.msi",
    );
    expect(screen.getByRole("link", { name: "Download DEB" }).getAttribute("href")).toContain(
      "meerkateer-controller_0.2.0_amd64.deb",
    );
    const ubuntuInstallCommand = screen.getByLabelText("Ubuntu install command").textContent;
    expect(ubuntuInstallCommand).toContain("curl --fail --location --remote-name");
    expect(ubuntuInstallCommand).toContain(
      "sha256sum --check meerkateer-controller_0.2.0_amd64.deb.sha256",
    );
    expect(ubuntuInstallCommand).toContain(
      "sudo apt install ./meerkateer-controller_0.2.0_amd64.deb",
    );
    expect(windowsCliLink.getAttribute("href")).toContain(
      "meerkateer-controller-windows-x86_64.zip",
    );
    expect(screen.getByText(/The service stays quiet and headless/)).toBeTruthy();
    expect(screen.getByText(/first enrollment requires explicit authorization/)).toBeTruthy();
    expect(screen.getByText("Local / self-hosted Community")).toBeTruthy();
    expect(screen.getByText(/Visible in setup, but disabled/)).toBeTruthy();
    expect(
      screen.getByRole("heading", { name: "Find the broken step, not just “connection failed.”" }),
    ).toBeTruthy();
    expect(screen.getByText(/unwritable\/full disk, DNS/)).toBeTruthy();
    expect(screen.getAllByRole("link", { name: /Road to 1.0/ })[0].getAttribute("href")).toBe(
      "/roadmap",
    );
    expect(screen.queryByRole("heading", { name: "Community Alpha" })).toBeNull();
    expect(
      Array.from(container.querySelectorAll<HTMLImageElement>(".capability-friend img")).map(
        (image) => image.getAttribute("src"),
      ),
    ).toEqual([
      "/friends/machine-scout.png",
      "/friends/heartbeat-keeper.png",
      "/friends/timeline-guide.png",
      "/friends/workspace-organizer.png",
      "/friends/state-watcher.png",
      "/friends/access-guardian.png",
    ]);
    expect(screen.queryByText("Admin token")).toBeNull();
    expect(fetch).not.toHaveBeenCalled();
  });

  it("keeps the complete roadmap on a dedicated single-purpose route", () => {
    window.history.replaceState({}, "", "/roadmap");
    const fetch = vi.fn();
    vi.stubGlobal("fetch", fetch);
    const { container } = render(<App />);

    expect(
      screen.getByRole("heading", { name: "Built carefully. Proven step by step." }),
    ).toBeTruthy();
    expect(
      screen.getByRole("heading", { name: "Ten milestones, one dependable product." }),
    ).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Community Alpha" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "No Cloud fork" })).toBeTruthy();
    expect(screen.getAllByRole("article")).toHaveLength(12);
    expect(container.querySelectorAll(".phase-card")).toHaveLength(10);
    expect(container.querySelectorAll(".phase-timeline-marker")).toHaveLength(10);
    expect(
      screen.getByText(/Failure → evidence → one incident → alert → fresh recovery completes/),
    ).toBeTruthy();
    expect(fetch).not.toHaveBeenCalled();
  });

  it("keeps the public root separate from first-run setup", () => {
    window.history.replaceState({}, "", "/");
    const fetch = vi.fn();
    vi.stubGlobal("fetch", fetch);
    render(<App />);
    expect(
      screen.getByRole("heading", { name: "Know what broke, and when it recovered." }),
    ).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Create your company" })).toBeNull();
    expect(fetch).not.toHaveBeenCalled();
  });

  it("gives the planned Cloud route its own clear managed identity", () => {
    window.history.replaceState({}, "", "/cloud");
    const fetch = vi.fn();
    vi.stubGlobal("fetch", fetch);
    const { container } = render(<App />);
    expect(screen.getByRole("link", { name: "Meerkateer Cloud home" })).toBeTruthy();
    expect(
      screen.getByRole("heading", { name: "Your servers, watched from the cloud." }),
    ).toBeTruthy();
    expect(screen.getByText("Hosted Beta planned after v0.5 Operations Beta")).toBeTruthy();
    expect(screen.getByText("Isolated companies and workspaces")).toBeTruthy();
    expect(screen.getByText(/Stripe remains off/)).toBeTruthy();
    expect(screen.getByAltText("Meerkateer mascot standing above a friendly cloud")).toBeTruthy();
    expect(container.querySelector(".cloud-sky")?.getAttribute("aria-hidden")).toBe("true");
    expect(fetch).not.toHaveBeenCalled();
  });

  it("opens first setup inside the single Community access screen", () => {
    window.history.replaceState({}, "", "/setup");
    const fetch = vi.fn();
    vi.stubGlobal("fetch", fetch);
    const { container } = render(<App />);
    expect(screen.getByRole("heading", { name: "Open your company." })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Create your company" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "First setup" }).getAttribute("aria-selected")).toBe(
      "true",
    );
    expect(screen.getByRole("link", { name: /Open the self-hosting guide/ })).toBeTruthy();
    expect(container.querySelector(".status-grid")).toBeNull();
    expect(fetch).not.toHaveBeenCalled();
  });

  it("switches sign in, first setup, and recovery without duplicate pages", () => {
    window.history.replaceState({}, "", "/login");
    vi.stubGlobal("fetch", vi.fn());
    render(<App />);

    expect(screen.getByRole("heading", { name: "Sign in to your company" })).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "First setup" }));
    expect(screen.getByRole("heading", { name: "Create your company" })).toBeTruthy();
    expect(window.location.pathname).toBe("/login");
    expect(window.location.search).toBe("?mode=setup");

    fireEvent.click(screen.getByRole("tab", { name: "Recover" }));
    expect(screen.getByRole("heading", { name: "Recover owner access" })).toBeTruthy();
    expect(window.location.search).toBe("?mode=recover");
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
      "/login?mode=recover",
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
    window.history.replaceState({}, "", "/app/services/service-deep-link");
    vi.stubGlobal(
      "fetch",
      vi.fn((input: string | URL | Request) =>
        Promise.resolve(String(input) === "/health" ? json(health) : json({}, 401)),
      ),
    );
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Open your company." })).toBeTruthy();
    expect(screen.getByLabelText("Owner email")).toBeTruthy();
    expect(screen.getByText("After sign in, you will return to the page you opened.")).toBeTruthy();
    expect(screen.queryByText("Control plane")).toBeNull();
  });

  it("reports an unavailable API without leaking details", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("network unavailable")));
    render(<App />);
    expect(await screen.findByText("API unavailable")).toBeTruthy();
    expect(screen.getAllByRole("alert").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Retry connection" })).toBeTruthy();
  });

  it("guides a new owner to create the first workspace before connecting a system", async () => {
    window.history.replaceState({}, "", "/app/integrations");
    vi.stubGlobal("fetch", emptyConsoleFetch("owner"));
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Connect your systems" })).toBeTruthy();
    expect(screen.getByText("Create the first workspace")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Name your first workspace" })).toBeTruthy();
    expect(screen.getByText("Workspace ready").closest("li")?.getAttribute("aria-current")).toBe(
      "step",
    );
    expect((screen.getByLabelText("Active workspace") as HTMLSelectElement).disabled).toBe(true);
  });

  it("keeps management actions unavailable to a viewer on a direct deep link", async () => {
    window.history.replaceState({}, "", "/app/integrations");
    vi.stubGlobal("fetch", emptyConsoleFetch("viewer"));
    const connect = render(<App />);
    expect(await screen.findByText("Administrator access required")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Connect a machine/ })).toBeNull();
    expect(screen.queryByRole("button", { name: "Issue enrollment token" })).toBeNull();
    expect(screen.queryByRole("heading", { name: "Test your alert channel" })).toBeNull();

    connect.unmount();
    window.history.replaceState({}, "", "/app/alerts");
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Alerts" })).toBeTruthy();
    expect(screen.getByText("Administrator access required")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Send test alert" })).toBeNull();
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
        if (path === "/v1/alerts/policy") {
          return Promise.resolve(
            json({
              enabled: true,
              notify_down: true,
              notify_recovered: true,
              cooldown_seconds: 0,
              webhook_configured: false,
              updated_at: null,
            }),
          );
        }
        if (path === "/v1/admin/summary") {
          return Promise.resolve(
            json({
              tenant_id: "00000000-0000-4000-8000-000000000001",
              deployment_mode: "community",
              projects: 1,
              services: 1,
              agents: 1,
              open_incidents: 1,
              pending_alerts: 0,
              dead_lettered_alerts: 0,
              active_maintenance_windows: 0,
              oldest_pending_alert_at: null,
              worker_status: "healthy",
              worker_started_at: "2026-09-28T00:00:00Z",
              worker_last_cycle_at: "2026-09-28T00:02:00Z",
              worker_last_cycle_claimed: 3,
              worker_last_cycle_completed: 2,
              worker_last_cycle_retried: 1,
              worker_last_cycle_dead_lettered: 0,
            }),
          );
        }
        if (path.startsWith("/v1/incidents?")) {
          return Promise.resolve(
            json({
              items: [
                {
                  id: "00000000-0000-4000-8000-000000000009",
                  project_id: "00000000-0000-4000-8000-000000000003",
                  service_id: "00000000-0000-4000-8000-000000000004",
                  service: "game-api",
                  status: "open",
                  severity: "critical",
                  title: "game-api is offline",
                  cause: "game process exited",
                  started_at: "2026-09-28T00:02:00Z",
                  last_observed_at: "2026-09-28T00:02:00Z",
                  resolved_at: null,
                  acknowledged_at: null,
                  acknowledged_by: null,
                  assigned_to: null,
                  assignee: null,
                },
              ],
            }),
          );
        }
        if (path.startsWith("/v1/alerts/deliveries?")) {
          return Promise.resolve(
            json({
              items: [
                {
                  id: "00000000-0000-4000-8000-000000000010",
                  project_id: "00000000-0000-4000-8000-000000000003",
                  service_id: "00000000-0000-4000-8000-000000000004",
                  service: "game-api",
                  incident_id: "00000000-0000-4000-8000-000000000009",
                  replay_of: null,
                  transition: "down",
                  status: "dead_lettered",
                  observed_at: "2026-09-28T00:02:00Z",
                  created_at: "2026-09-28T00:02:01Z",
                  delivered_at: null,
                  attempts: 5,
                  last_error: "receiver unavailable",
                  suppression_reason: null,
                },
              ],
            }),
          );
        }
        if (
          path.startsWith("/v1/incidents/activity?") ||
          path.startsWith("/v1/maintenance-windows?") ||
          path.startsWith("/v1/audit-events?")
        ) {
          return Promise.resolve(json({ items: [] }));
        }
        if (path === "/v1/agents/00000000-0000-4000-8000-000000000006/telemetry") {
          return Promise.resolve(
            json({
              agent_id: "00000000-0000-4000-8000-000000000006",
              connection_state: "online",
              collection_state: "complete",
              observed_at: "2026-09-28T00:02:00Z",
              received_at: "2026-09-28T00:02:01Z",
              snapshot_stale: false,
              platform: "linux",
              architecture: "x86_64",
              cpu_usage_percent: 25,
              memory: {
                used_bytes: 8589934592,
                total_bytes: 17179869184,
                utilization_percent: 50,
              },
              disk: {
                used_bytes: 80530636800,
                total_bytes: 107374182400,
                utilization_percent: 75,
              },
              inodes: {
                used: 250000,
                total: 1000000,
                utilization_percent: 25,
              },
              processes: [
                { name: "java", running: false, instances: 0 },
                { name: "postgres", running: true, instances: 1 },
              ],
              services: [
                { name: "minecraft.service", running: false, state: "failed" },
                { name: "postgresql.service", running: true, state: "active" },
                { name: "docker.service", running: null, state: "permission_denied" },
              ],
              missing_metrics: [],
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
        if (path.endsWith("/enrollment-tokens")) {
          return Promise.resolve(
            json({
              token_id: "00000000-0000-4000-8000-000000000008",
              project_id: "00000000-0000-4000-8000-000000000003",
              secret: "enroll_test_secret",
              expires_at: "2026-09-28T00:12:00Z",
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
    const overview = render(<App />);
    expect(await screen.findByText("game process exited")).toBeTruthy();
    expect(screen.getByText("Service reported offline")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Arena" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Everything at a glance" })).toBeTruthy();
    expect(document.querySelector('a[href="/app"][aria-current="page"]')).toBeTruthy();
    expect(screen.getAllByRole("link", { name: /Machines/ })[0]?.getAttribute("href")).toBe(
      "/app/machines",
    );

    overview.unmount();
    window.history.replaceState({}, "", "/app/machines");
    const machines = render(<App />);
    expect(await screen.findByRole("heading", { name: "Machines" })).toBeTruthy();
    expect(screen.getByText("Arena Host 01")).toBeTruthy();
    expect(screen.getByText("1 online")).toBeTruthy();
    expect(document.querySelector('a[href="/app/machines"][aria-current="page"]')).toBeTruthy();
    expect(screen.getByRole("link", { name: "Arena Host 01" }).getAttribute("href")).toBe(
      "/app/machines/00000000-0000-4000-8000-000000000006",
    );
    fireEvent.change(screen.getByLabelText("Search machines"), {
      target: { value: "not-a-real-machine" },
    });
    expect(screen.getByText("No machines match these filters.")).toBeTruthy();

    machines.unmount();
    window.history.replaceState({}, "", "/app/machines/00000000-0000-4000-8000-000000000006");
    const machineDetail = render(<App />);
    expect(await screen.findByRole("heading", { name: "Arena Host 01" })).toBeTruthy();
    expect(screen.getByText("Machine detail")).toBeTruthy();
    expect(await screen.findByText("What the agent can see")).toBeTruthy();
    expect(screen.getByText("1 watched process is not running.")).toBeTruthy();
    expect(screen.getByText("8.0 GiB / 16 GiB")).toBeTruthy();
    expect(screen.getByText("250,000 / 1,000,000")).toBeTruthy();
    expect(screen.getAllByText("java")).toHaveLength(2);
    expect(screen.getByText("Not running")).toBeTruthy();
    expect(screen.getByText("postgres")).toBeTruthy();
    expect(screen.getByText("1 instance")).toBeTruthy();
    expect(screen.getByText("minecraft.service")).toBeTruthy();
    expect(screen.getByText("Failed")).toBeTruthy();
    expect(screen.getByText("postgresql.service")).toBeTruthy();
    expect(screen.getByText("Running")).toBeTruthy();
    expect(screen.getByText("docker.service")).toBeTruthy();
    expect(screen.getByText("Permission denied")).toBeTruthy();
    expect(screen.getByRole("link", { name: "Arena Host 01" }).getAttribute("aria-current")).toBe(
      "page",
    );

    machineDetail.unmount();
    window.history.replaceState({}, "", "/app/machines/not-owned-by-this-workspace");
    const unknownMachine = render(<App />);
    expect(await screen.findByText("Machine not found in this workspace.")).toBeTruthy();

    unknownMachine.unmount();
    window.history.replaceState({}, "", "/app/services");
    const services = render(<App />);
    expect(await screen.findByRole("heading", { name: "Services" })).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Search services"), {
      target: { value: "not-a-real-service" },
    });
    expect(screen.getByText("No services match these filters.")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Search services"), { target: { value: "game" } });
    expect(screen.getAllByText("game-api").length).toBeGreaterThan(0);
    expect(document.querySelector(".service-button")?.getAttribute("href")).toBe(
      "/app/services/00000000-0000-4000-8000-000000000004",
    );

    services.unmount();
    window.history.replaceState({}, "", "/app/services/00000000-0000-4000-8000-000000000004");
    const serviceDetail = render(<App />);
    expect(await screen.findByRole("heading", { name: "Services" })).toBeTruthy();
    expect(
      document
        .querySelector('.service-button[href$="00000000-0000-4000-8000-000000000004"]')
        ?.getAttribute("aria-current"),
    ).toBe("page");

    serviceDetail.unmount();
    window.history.replaceState({}, "", "/app/services/not-owned-by-this-workspace");
    const unknownService = render(<App />);
    expect(await screen.findByText("Service not found in this workspace.")).toBeTruthy();
    expect(document.querySelector(".service-button[aria-current='page']")).toBeNull();

    unknownService.unmount();
    window.history.replaceState({}, "", "/app/incidents/00000000-0000-4000-8000-000000000004");
    const incidents = render(<App />);
    expect(await screen.findByRole("heading", { name: "Incidents" })).toBeTruthy();
    expect(screen.getByText(/game process exited/)).toBeTruthy();
    expect(screen.getByText("game-api is offline")).toBeTruthy();
    expect(screen.getByText("Not acknowledged")).toBeTruthy();
    expect(screen.getByText("Unassigned")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Acknowledge" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Assign to me" })).toBeTruthy();
    expect(screen.getByLabelText("Add operator note")).toBeTruthy();

    incidents.unmount();
    window.history.replaceState({}, "", "/app/alerts");
    const alerts = render(<App />);
    expect(await screen.findByRole("heading", { name: "Alerts" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "One installation webhook" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Test your alert channel" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Transition notifications" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Delivery outcomes" })).toBeTruthy();
    expect(screen.getByRole("spinbutton", { name: /Repeat-down cooldown/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Replay" })).toBeTruthy();

    alerts.unmount();
    window.history.replaceState({}, "", "/app/admin");
    const admin = render(<App />);
    expect(await screen.findByRole("heading", { name: "Administration" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Worker is healthy" })).toBeTruthy();
    expect(screen.getByText(/3 claimed · 2 completed · 1 retried/)).toBeTruthy();

    admin.unmount();
    window.history.replaceState({}, "", "/app/integrations");
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Connect your systems" })).toBeTruthy();
    expect(
      screen.getByRole("heading", { name: "Four small steps to useful monitoring" }),
    ).toBeTruthy();
    expect(screen.getByText("Fresh signal received")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Connect a machine/ }));
    expect(screen.getByText("Machine connection steps")).toBeTruthy();
    // biome-ignore lint/suspicious/noDocumentCookie: this integration test exercises the production CSRF reader.
    document.cookie = "meerkateer_csrf=test-csrf; path=/";
    fireEvent.click(screen.getByRole("button", { name: "Issue enrollment token" }));
    expect(await screen.findByText("enroll_test_secret")).toBeTruthy();
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: vi.fn().mockRejectedValue(new Error("blocked")) },
    });
    fireEvent.click(screen.getByRole("button", { name: "Copy token" }));
    expect(await screen.findByText(/Copy was blocked by the browser/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Connect an application/ }));
    expect(screen.getByText("Application connection steps")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Create process + SDK key" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Rust" }).getAttribute("href")).toBe("/docs/rust-sdk");
    expect(screen.queryByRole("heading", { name: "Test your alert channel" })).toBeNull();
  });
});
