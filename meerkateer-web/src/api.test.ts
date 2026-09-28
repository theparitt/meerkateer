import { afterEach, describe, expect, it, vi } from "vitest";
import {
  assignWorkspaceAgent,
  createService,
  createWorkspace,
  issueServiceCredential,
  issueWorkspaceEnrollmentToken,
  unassignWorkspaceAgent,
} from "./api";

const projectId = "00000000-0000-4000-8000-000000000003";
const agentId = "00000000-0000-4000-8000-000000000006";

function json(body: unknown, status: number) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function setDocumentCookie(value: string) {
  Object.defineProperty(document, "cookie", { configurable: true, value });
}

describe("management API", () => {
  afterEach(() => {
    setDocumentCookie("");
    vi.unstubAllGlobals();
  });

  it("sends CSRF proof for workspace and machine mutations", async () => {
    setDocumentCookie("meerkateer_csrf=csrf-proof");
    const fetchMock = vi.fn((input: string | URL | Request, init?: RequestInit) => {
      const path = String(input);
      if (path === "/v1/projects" && init?.method === "POST") {
        return Promise.resolve(
          json(
            {
              id: projectId,
              slug: "arena-ops",
              display_name: "Arena Ops",
              created_at: "2026-09-28T00:00:00Z",
            },
            201,
          ),
        );
      }
      if (path.endsWith("/enrollment-tokens")) {
        return Promise.resolve(
          json(
            {
              token_id: "00000000-0000-4000-8000-000000000007",
              project_id: projectId,
              prefix: "abcdef123456",
              secret: "mka_enroll_once",
              expires_at: "2026-09-28T00:10:00Z",
            },
            201,
          ),
        );
      }
      return Promise.resolve(new Response(null, { status: 204 }));
    });
    vi.stubGlobal("fetch", fetchMock);

    await createWorkspace({ slug: "arena-ops", display_name: "Arena Ops" });
    const token = await issueWorkspaceEnrollmentToken(projectId);
    await assignWorkspaceAgent(projectId, agentId);
    await unassignWorkspaceAgent(projectId, agentId);

    expect(token.secret).toBe("mka_enroll_once");
    expect(fetchMock).toHaveBeenCalledTimes(4);
    for (const [, init] of fetchMock.mock.calls) {
      expect(init?.credentials).toBe("same-origin");
      expect(init?.headers).toMatchObject({ "X-Meerkateer-CSRF": "csrf-proof" });
    }
    expect(fetchMock.mock.calls[2][1]?.method).toBe("PUT");
    expect(fetchMock.mock.calls[3][1]?.method).toBe("DELETE");
  });

  it("fails closed before a mutation when the CSRF cookie is missing", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    await expect(createWorkspace({ slug: "arena-ops", display_name: "Arena Ops" })).rejects.toThrow(
      "missing CSRF protection",
    );
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("creates a process and issues its one-time SDK key", async () => {
    setDocumentCookie("meerkateer_csrf=csrf-proof");
    const fetchMock = vi.fn((input: string | URL | Request) => {
      const path = String(input);
      if (path.includes("/projects/") && path.endsWith("/services")) {
        return Promise.resolve(
          json(
            {
              id: "00000000-0000-4000-8000-000000000008",
              project_id: projectId,
              slug: "game-server-01",
              environment: "production",
              created_at: "2026-09-28T00:00:00Z",
              status: {
                state: "unknown",
                reported_state: null,
                stale: false,
                last_sequence: null,
                observed_at: null,
                updated_at: null,
              },
            },
            201,
          ),
        );
      }
      return Promise.resolve(
        json(
          {
            credential_id: "00000000-0000-4000-8000-000000000009",
            prefix: "abcdef123456",
            secret: "mks_sk_once",
          },
          201,
        ),
      );
    });
    vi.stubGlobal("fetch", fetchMock);

    const process = await createService(projectId, {
      slug: "game-server-01",
      environment: "production",
    });
    const credential = await issueServiceCredential(process.id);

    expect(process.slug).toBe("game-server-01");
    expect(credential.secret).toBe("mks_sk_once");
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });
});
