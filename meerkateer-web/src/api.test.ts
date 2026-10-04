import { afterEach, describe, expect, it, vi } from "vitest";
import {
  acceptMemberInvitation,
  acceptMemberPasswordReset,
  assignWorkspaceAgent,
  cancelMemberInvitation,
  changePassword,
  createMemberInvitation,
  createMemberPasswordReset,
  createService,
  createWorkspace,
  declineMemberInvitation,
  inspectMemberInvitation,
  inspectMemberPasswordReset,
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

  it("keeps internal invitation secrets in POST bodies and applies CSRF only to creation", async () => {
    setDocumentCookie("meerkateer_csrf=csrf-proof");
    const fetchMock = vi.fn((input: string | URL | Request, init?: RequestInit) => {
      const path = String(input);
      if (path === "/v1/members/invitations") {
        return Promise.resolve(
          json(
            {
              invitation: {
                id: "00000000-0000-4000-8000-000000000010",
                username: "nina_ops",
                display_name: "Nina",
                role: "operator",
                expires_at: "2026-10-05T00:00:00Z",
                created_at: "2026-10-04T00:00:00Z",
                status: "pending",
              },
              secret: "mki_private_once",
            },
            201,
          ),
        );
      }
      if (path.endsWith("/inspect")) {
        return Promise.resolve(
          json(
            {
              company: "Acme Games",
              username: "nina_ops",
              display_name: "Nina",
              role: "operator",
              expires_at: "2026-10-05T00:00:00Z",
            },
            200,
          ),
        );
      }
      if (path.endsWith("/accept")) {
        return Promise.resolve(
          json(
            {
              tenant_id: "00000000-0000-4000-8000-000000000001",
              user_id: "00000000-0000-4000-8000-000000000011",
              role: "operator",
              email: "internal@internal.meerkateer.invalid",
              username: "nina_ops",
              display_name: "Nina",
            },
            200,
          ),
        );
      }
      expect(init?.method).toBe("POST");
      return Promise.resolve(new Response(null, { status: 204 }));
    });
    vi.stubGlobal("fetch", fetchMock);

    const issued = await createMemberInvitation({
      username: "nina_ops",
      display_name: "Nina",
      role: "operator",
      expires_in_seconds: 86_400,
    });
    const preview = await inspectMemberInvitation(issued.secret);
    const session = await acceptMemberInvitation(issued.secret, "member-test-password-123");
    await declineMemberInvitation("mki_second_secret");

    expect(preview.company).toBe("Acme Games");
    expect(session.username).toBe("nina_ops");
    expect(fetchMock).toHaveBeenCalledTimes(4);
    expect(fetchMock.mock.calls[0][1]?.headers).toMatchObject({
      "X-Meerkateer-CSRF": "csrf-proof",
    });
    for (const [path, init] of fetchMock.mock.calls.slice(1)) {
      expect(String(path)).not.toContain("mki_");
      expect(String(init?.body)).toContain("mki_");
      expect(init?.headers).not.toHaveProperty("X-Meerkateer-CSRF");
    }
  });

  it("keeps password-reset tokens out of URLs and protects administrator and account mutations", async () => {
    setDocumentCookie("meerkateer_csrf=csrf-proof");
    const fetchMock = vi.fn((input: string | URL | Request, _init?: RequestInit) => {
      const path = String(input);
      if (path.endsWith("/password-reset-links")) {
        return Promise.resolve(
          json({ secret: "mkr_private_once", expires_at: "2026-10-05T00:00:00Z" }, 201),
        );
      }
      if (path === "/v1/member-password-resets/inspect") {
        return Promise.resolve(
          json(
            {
              company: "Acme Games",
              username: "nina_ops",
              display_name: "Nina",
              expires_at: "2026-10-05T00:00:00Z",
            },
            200,
          ),
        );
      }
      if (path === "/v1/member-password-resets/accept") {
        return Promise.resolve(
          json(
            {
              tenant_id: "00000000-0000-4000-8000-000000000001",
              user_id: "00000000-0000-4000-8000-000000000011",
              role: "operator",
              email: "internal@internal.meerkateer.invalid",
              username: "nina_ops",
              display_name: "Nina",
            },
            200,
          ),
        );
      }
      return Promise.resolve(new Response(null, { status: 204 }));
    });
    vi.stubGlobal("fetch", fetchMock);

    const reset = await createMemberPasswordReset("member-id", 3_600);
    await inspectMemberPasswordReset(reset.secret);
    await acceptMemberPasswordReset(reset.secret, "member-new-password-123");
    await cancelMemberInvitation("invitation-id");
    await changePassword({
      current_password: "member-current-password-123",
      new_password: "member-new-password-456",
    });

    expect(fetchMock).toHaveBeenCalledTimes(5);
    for (const [path] of fetchMock.mock.calls) expect(String(path)).not.toContain("mkr_");
    expect(fetchMock.mock.calls[0][1]?.headers).toMatchObject({
      "X-Meerkateer-CSRF": "csrf-proof",
    });
    expect(fetchMock.mock.calls[1][1]?.headers).not.toHaveProperty("X-Meerkateer-CSRF");
    expect(fetchMock.mock.calls[2][1]?.headers).not.toHaveProperty("X-Meerkateer-CSRF");
    expect(fetchMock.mock.calls[3][1]?.headers).toMatchObject({
      "X-Meerkateer-CSRF": "csrf-proof",
    });
    expect(fetchMock.mock.calls[4][1]?.headers).toMatchObject({
      "X-Meerkateer-CSRF": "csrf-proof",
    });
  });
});
