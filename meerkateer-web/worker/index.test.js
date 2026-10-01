import { afterEach, describe, expect, it, vi } from "vitest";

import worker, { apiOrigin, isApiPath } from "./index.js";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("Cloudflare web gateway", () => {
  it("recognizes only Meerkateer API paths", () => {
    expect(isApiPath("/v1/session")).toBe(true);
    expect(isApiPath("/live")).toBe(true);
    expect(isApiPath("/health")).toBe(true);
    expect(isApiPath("/app")).toBe(false);
  });

  it("requires a separate HTTPS API origin", () => {
    expect(() => apiOrigin("http://api.example.com", new URL("https://web.example.com"))).toThrow();
    expect(() =>
      apiOrigin("https://web.example.com", new URL("https://web.example.com")),
    ).toThrow();
    expect(apiOrigin("https://api.example.com", new URL("https://web.example.com")).origin).toBe(
      "https://api.example.com",
    );
  });

  it("serves static routes through the assets binding", async () => {
    const assetResponse = new Response("asset");
    const assets = { fetch: vi.fn().mockResolvedValue(assetResponse) };
    const request = new Request("https://web.example.com/app");
    const response = await worker.fetch(request, { ASSETS: assets });
    expect(response).toBe(assetResponse);
    expect(assets.fetch).toHaveBeenCalledWith(request);
  });

  it("proxies API traffic without accepting spoofed client-address headers", async () => {
    const upstream = vi
      .fn()
      .mockResolvedValue(
        new Response("ok", { headers: { "Set-Cookie": "meerkateer_session=test; Secure" } }),
      );
    vi.stubGlobal("fetch", upstream);
    const request = new Request("https://web.example.com/v1/session?view=current", {
      headers: {
        Cookie: "meerkateer_session=test",
        "CF-Access-Client-Id": "untrusted-client",
        "CF-Access-Client-Secret": "untrusted-secret",
        "CF-Connecting-IP": "192.0.2.2",
        "X-Forwarded-For": "192.0.2.3",
      },
    });
    const response = await worker.fetch(request, {
      ASSETS: { fetch: vi.fn() },
      MEERKATEER_API_ORIGIN: "https://api.example.com",
    });
    const forwarded = upstream.mock.calls[0][0];
    expect(forwarded.url).toBe("https://api.example.com/v1/session?view=current");
    expect(forwarded.headers.get("cookie")).toBe("meerkateer_session=test");
    expect(forwarded.headers.get("cf-connecting-ip")).toBeNull();
    expect(forwarded.headers.get("cf-access-client-id")).toBeNull();
    expect(forwarded.headers.get("cf-access-client-secret")).toBeNull();
    expect(forwarded.headers.get("x-forwarded-for")).toBeNull();
    expect(forwarded.headers.get("x-meerkateer-proxy")).toBe("cloudflare-worker");
    expect(response.headers.get("set-cookie")).toContain("meerkateer_session=test");
  });

  it("fails closed when the API origin is missing", async () => {
    const response = await worker.fetch(new Request("https://web.example.com/v1/session"), {
      ASSETS: { fetch: vi.fn() },
    });
    expect(response.status).toBe(503);
    await expect(response.json()).resolves.toEqual({ code: "api_origin_not_configured" });
  });

  it("injects a configured Cloudflare Access service token", async () => {
    const upstream = vi.fn().mockResolvedValue(new Response("ok"));
    vi.stubGlobal("fetch", upstream);
    await worker.fetch(new Request("https://web.example.com/health"), {
      ASSETS: { fetch: vi.fn() },
      MEERKATEER_API_ORIGIN: "https://api.example.com",
      MEERKATEER_ACCESS_CLIENT_ID: "trusted-client",
      MEERKATEER_ACCESS_CLIENT_SECRET: "trusted-secret",
    });
    const forwarded = upstream.mock.calls[0][0];
    expect(forwarded.headers.get("cf-access-client-id")).toBe("trusted-client");
    expect(forwarded.headers.get("cf-access-client-secret")).toBe("trusted-secret");
  });

  it("fails closed when only half of the Access service token is configured", async () => {
    const response = await worker.fetch(new Request("https://web.example.com/health"), {
      ASSETS: { fetch: vi.fn() },
      MEERKATEER_API_ORIGIN: "https://api.example.com",
      MEERKATEER_ACCESS_CLIENT_ID: "incomplete",
    });
    expect(response.status).toBe(503);
    await expect(response.json()).resolves.toEqual({ code: "api_access_not_configured" });
  });
});
