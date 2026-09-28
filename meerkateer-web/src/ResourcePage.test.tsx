import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { isResourcePath } from "./ResourcePage";

afterEach(cleanup);

describe("public resource links", () => {
  it("resolves every footer link to a real section or page", () => {
    window.history.replaceState({}, "", "/");
    const { container } = render(<App />);
    for (const link of container.querySelectorAll<HTMLAnchorElement>(".public-footer a")) {
      const href = link.getAttribute("href");
      expect(href, link.textContent ?? "footer link").toBeTruthy();
      if (!href) continue;
      if (href.startsWith("#") || href.startsWith("/#")) {
        expect(container.querySelector(href.replace(/^\//, "")), href).toBeTruthy();
      } else if (href.startsWith("https://")) {
        expect(href).toBe("https://github.com/theparitt/meerkateer");
      } else {
        expect(["/", "/cloud"].includes(href) || isResourcePath(href), href).toBe(true);
      }
    }
  });

  it("links the header directly to the GitHub repository with its icon", () => {
    window.history.replaceState({}, "", "/");
    const { container } = render(<App />);
    const link = container.querySelector<HTMLAnchorElement>(".public-nav .github-link");
    expect(link?.getAttribute("href")).toBe("https://github.com/theparitt/meerkateer");
    expect(link?.querySelector("svg")).toBeTruthy();
  });

  it("shows a visual system map, mascot actions, and working code examples", async () => {
    window.history.replaceState({}, "", "/");
    const { container } = render(<App />);
    expect(screen.getByRole("heading", { name: "Little signals. One clear story." })).toBeTruthy();
    expect(screen.getByText("Host agent sends telemetry")).toBeTruthy();
    expect(screen.getByText("SDK sends heartbeats and events")).toBeTruthy();
    expect(screen.getByRole("link", { name: "Download diagram ↓" }).getAttribute("href")).toBe(
      "/meerkateer-system-map-v1.png",
    );
    expect(container.querySelectorAll(".story-art img")).toHaveLength(3);
    expect(screen.getByText(/await watch\.heartbeat\("ok"/)).toBeTruthy();
    expect(container.querySelector(".public-nav a[href='/#sdks']")).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "Go SDK" }));
    expect(screen.getByText(/meerkateer.FromEnv/)).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "Python SDK" }));
    expect(screen.getByText(/watch\.heartbeat\("ok"/)).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "PHP SDK" }));
    expect(screen.getByText(/Client::fromEnv/)).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "Host agent" }));
    expect(screen.getByText(/meerkateer-agent -- enroll/)).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "Rust SDK" }));
    expect(screen.getByText(/HeartbeatStatus::Ok/)).toBeTruthy();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
    fireEvent.click(screen.getByRole("button", { name: "Copy Rust SDK example" }));
    expect(await screen.findByText("Copied ✓")).toBeTruthy();
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("HeartbeatStatus::Ok"));
  });

  it.each([
    ["/docs", "Documentation"],
    ["/docs/node-sdk", "Node.js SDK"],
    ["/docs/go-sdk", "Go SDK"],
    ["/docs/php-sdk", "PHP SDK"],
    ["/docs/python-sdk", "Python SDK"],
    ["/docs/rust-sdk", "Rust SDK"],
    ["/get-started", "Getting started"],
    ["/help", "Help"],
    ["/source", "Source code"],
    ["/docs/phase-status", "Phase status"],
    ["/docs/commercial-readiness-plan", "Commercial readiness plan"],
    ["/docs/game-server-beta", "Game server beta"],
    ["/changelog", "Changelog"],
    ["/contributing", "Contributing"],
    ["/license", "Apache 2.0 license"],
    ["/security", "Security policy"],
    ["/governance", "Governance"],
  ])("renders %s with its project content", (path, title) => {
    window.history.replaceState({}, "", path);
    render(<App />);
    expect(screen.getByRole("heading", { name: title, level: 1 })).toBeTruthy();
    expect(screen.getByText("Meerkateer · Developer preview")).toBeTruthy();
  });
});
