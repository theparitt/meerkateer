import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
    const fetchMock = vi.fn().mockResolvedValue(
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
    vi.stubGlobal("fetch", fetchMock);
    render(<NetworkProbePanel service={service} />);
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
    expect(fetchMock.mock.calls[0][0]).toBe(`/v1/services/${service.id}/network-probe`);
    expect(fetchMock.mock.calls[0][1].headers).toMatchObject({
      "X-Meerkateer-CSRF": "test-proof",
    });
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toMatchObject({
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
      vi
        .fn()
        .mockResolvedValue(
          new Response(JSON.stringify({ code: "unsafe_probe_destination" }), { status: 400 }),
        ),
    );
    render(<NetworkProbePanel service={service} />);
    fireEvent.change(screen.getByLabelText("Public hostname or IP"), {
      target: { value: "127.0.0.1" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Run check" }));
    expect(await screen.findByText(/blocks it to protect your network/)).toBeTruthy();
  });
});
