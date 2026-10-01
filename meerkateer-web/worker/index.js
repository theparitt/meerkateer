const API_PATHS = new Set(["/live", "/health", "/ready", "/server-info", "/openapi.json"]);

export function isApiPath(pathname) {
  return pathname.startsWith("/v1/") || API_PATHS.has(pathname);
}

export function apiOrigin(value, publicUrl) {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error("MEERKATEER_API_ORIGIN is not configured");
  }
  const origin = new URL(value);
  if (
    origin.protocol !== "https:" ||
    origin.username ||
    origin.password ||
    origin.pathname !== "/" ||
    origin.search ||
    origin.hash
  ) {
    throw new Error("MEERKATEER_API_ORIGIN must be an HTTPS origin without a path");
  }
  if (origin.host === publicUrl.host) {
    throw new Error("MEERKATEER_API_ORIGIN must not point back to this Worker");
  }
  return origin;
}

function jsonError(status, code) {
  return Response.json(
    { code },
    {
      status,
      headers: {
        "Cache-Control": "no-store",
        "X-Content-Type-Options": "nosniff",
      },
    },
  );
}

export default {
  async fetch(request, env) {
    const publicUrl = new URL(request.url);
    if (!isApiPath(publicUrl.pathname)) return env.ASSETS.fetch(request);

    let origin;
    try {
      origin = apiOrigin(env.MEERKATEER_API_ORIGIN, publicUrl);
    } catch (error) {
      console.error(error instanceof Error ? error.message : "invalid API origin");
      return jsonError(503, "api_origin_not_configured");
    }

    const upstreamUrl = new URL(`${publicUrl.pathname}${publicUrl.search}`, origin);
    const upstreamRequest = new Request(upstreamUrl, request);
    upstreamRequest.headers.delete("cf-connecting-ip");
    upstreamRequest.headers.delete("cf-access-client-id");
    upstreamRequest.headers.delete("cf-access-client-secret");
    upstreamRequest.headers.delete("x-forwarded-for");
    upstreamRequest.headers.delete("x-real-ip");
    upstreamRequest.headers.set("X-Meerkateer-Proxy", "cloudflare-worker");

    const accessClientId = env.MEERKATEER_ACCESS_CLIENT_ID;
    const accessClientSecret = env.MEERKATEER_ACCESS_CLIENT_SECRET;
    if (Boolean(accessClientId) !== Boolean(accessClientSecret)) {
      return jsonError(503, "api_access_not_configured");
    }
    if (accessClientId && accessClientSecret) {
      upstreamRequest.headers.set("CF-Access-Client-Id", accessClientId);
      upstreamRequest.headers.set("CF-Access-Client-Secret", accessClientSecret);
    }

    try {
      return await fetch(upstreamRequest, { redirect: "manual" });
    } catch {
      return jsonError(502, "api_unavailable");
    }
  },
};
