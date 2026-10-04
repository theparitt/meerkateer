import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ServiceResponse } from "./api";
import { NetworkProbePanel } from "./NetworkProbePanel";

const service: ServiceResponse = {
  id: "00000000-0000-4000-8000-000000000004",
  project_id: "00000000-0000-4000-8000-000000000003",
  slug: "website",
  environment: "production",
  created_at: "2026-10-04T00:00:00Z",
  game: null,
  status: {
    state: "unknown",
    reported_state: null,
    stale: false,
    last_sequence: null,
    observed_at: null,
    updated_at: null,
  },
};

describe("manual network diagnostics", () => {
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it("sends a bounded HTTPS check and explains the certificate", async () => {
    Object.defineProperty(document, "cookie", {
      configurable: true,
      value: "meerkateer_csrf=test-proof",
    });
    const fetchMock = vi.fn().mockImplementation((input: RequestInfo | URL) => {
      const url = String(input);
      if (url.endsWith("/scheduled-probe"))
        return Promise.resolve(new Response(null, { status: 404 }));
      if (url.endsWith("/scheduled-probe/history")) {
        return Promise.resolve(new Response(JSON.stringify({ items: [] }), { status: 200 }));
      }
      return Promise.resolve(
        new Response(
          JSON.stringify({
            kind: "https",
            state: "responding",
            message: "The endpoint responded with the expected HTTP status.",
            probe_location: "community_control_plane",
            observed_at: "2026-10-04T01:02:03Z",
            response_ms: 31,
            status_code: 204,
            resolved_addresses: 2,
            tls_expires_at: "2027-01-01T00:00:00Z",
            tls_days_remaining: 88,
          }),
          { status: 200 },
        ),
      );
    });
    vi.stubGlobal("fetch", fetchMock);
    render(<NetworkProbePanel service={service} canManage />);
    fireEvent.change(screen.getByLabelText("Public hostname or IP"), {
      target: { value: "status.example.com" },
    });
    fireEvent.change(screen.getByLabelText("Expected status"), {
      target: { value: "204" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Run check" }));

    expect(await screen.findByText("responding")).toBeTruthy();
    expect(screen.getByText(/HTTP 204/)).toBeTruthy();
    expect(screen.getByText(/certificate 88 days remaining/)).toBeTruthy();
    const probeCall = fetchMock.mock.calls.find(([url]) => String(url).endsWith("/network-probe"));
    expect(probeCall?.[0]).toBe(`/v1/services/${service.id}/network-probe`);
    expect(probeCall?.[1].headers).toMatchObject({
      "X-Meerkateer-CSRF": "test-proof",
    });
    expect(JSON.parse(probeCall?.[1].body)).toMatchObject({
      kind: "https",
      host: "status.example.com",
      port: 443,
      path: "/",
      expected_status: 204,
      timeout_ms: 5000,
    });
  });

  it("turns an SSRF rejection into an operator-readable message", async () => {
    Object.defineProperty(document, "cookie", {
      configurable: true,
      value: "meerkateer_csrf=test-proof",
    });
    vi.stubGlobal(
      "fetch",
      vi.fn().mockImplementation((input: RequestInfo | URL) => {
        const url = String(input);
        if (url.endsWith("/scheduled-probe"))
          return Promise.resolve(new Response(null, { status: 404 }));
        if (url.endsWith("/scheduled-probe/history")) {
          return Promise.resolve(new Response(JSON.stringify({ items: [] }), { status: 200 }));
        }
        return Promise.resolve(
          new Response(JSON.stringify({ code: "unsafe_probe_destination" }), { status: 400 }),
        );
      }),
    );
    render(<NetworkProbePanel service={service} canManage />);
    fireEvent.change(screen.getByLabelText("Public hostname or IP"), {
      target: { value: "127.0.0.1" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Run check" }));
    expect(await screen.findByText(/blocks it to protect your network/)).toBeTruthy();
  });

  it("creates one debounced scheduled monitor from the same simple form", async () => {
    Object.defineProperty(document, "cookie", {
      configurable: true,
      value: "meerkateer_csrf=test-proof",
    });
    const scheduled = {
      id: "00000000-0000-4000-8000-000000000099",
      service_id: service.id,
      kind: "dns",
      host: "one.one.one.one",
      port: null,
      path: null,
      expected_status: null,
      timeout_ms: 5000,
      interval_seconds: 30,
      failure_threshold: 3,
      recovery_threshold: 2,
      enabled: true,
      consensus_state: "unknown",
      consecutive_failures: 0,
      consecutive_successes: 0,
      next_run_at: "2026-10-04T01:02:03Z",
      last_state: null,
      last_message: null,
      last_observed_at: null,
      last_response_ms: null,
      last_status_code: null,
      last_tls_expires_at: null,
    };
    const fetchMock = vi.fn().mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url.endsWith("/scheduled-probe/history")) {
        return Promise.resolve(new Response(JSON.stringify({ items: [] }), { status: 200 }));
      }
      if (url.endsWith("/scheduled-probe") && init?.method === "PUT") {
        return Promise.resolve(new Response(JSON.stringify(scheduled), { status: 200 }));
      }
      if (url.endsWith("/scheduled-probe")) {
        return Promise.resolve(new Response(null, { status: 404 }));
      }
      throw new Error(`Unexpected request: ${url}`);
    });
    vi.stubGlobal("fetch", fetchMock);
    render(<NetworkProbePanel service={service} canManage />);

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(2));

    fireEvent.change(screen.getByLabelText("Network check type"), {
      target: { value: "dns" },
    });
    fireEvent.change(screen.getByLabelText("Public hostname or IP"), {
      target: { value: "one.one.one.one" },
    });
    fireEvent.change(screen.getByLabelText("Scheduled check interval"), {
      target: { value: "30" },
    });
    fireEvent.change(screen.getByLabelText("Failure confirmation count"), {
      target: { value: "3" },
    });
    fireEvent.change(screen.getByLabelText("Recovery confirmation count"), {
      target: { value: "2" },
    });
    expect(
      (screen.getByRole("button", { name: "Start monitoring" }) as HTMLButtonElement).disabled,
    ).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Start monitoring" }));

    expect(await screen.findByText("Watching automatically")).toBeTruthy();
    const saveCall = fetchMock.mock.calls.find(
      ([url, init]) => String(url).endsWith("/scheduled-probe") && init?.method === "PUT",
    );
    expect(saveCall?.[1]?.headers).toMatchObject({
      "X-Meerkateer-CSRF": "test-proof",
    });
    expect(JSON.parse(String(saveCall?.[1]?.body))).toMatchObject({
      kind: "dns",
      host: "one.one.one.one",
      port: null,
      interval_seconds: 30,
      failure_threshold: 3,
      recovery_threshold: 2,
      enabled: true,
    });
  });
});
