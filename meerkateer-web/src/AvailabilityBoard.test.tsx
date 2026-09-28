import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { AvailabilityBoard } from "./AvailabilityBoard";
import type { ServiceResponse, TimelineItemResponse } from "./api";

const service: ServiceResponse = {
  id: "00000000-0000-4000-8000-000000000004",
  project_id: "00000000-0000-4000-8000-000000000003",
  slug: "game-api",
  environment: "production",
  created_at: "2026-09-26T00:00:00Z",
  game: null,
  status: {
    state: "online",
    reported_state: "online",
    stale: false,
    last_sequence: 3,
    observed_at: "2026-09-28T00:20:00Z",
    updated_at: "2026-09-28T00:20:01Z",
  },
};

const timeline: TimelineItemResponse[] = [
  {
    idempotency_key: "00000000-0000-4000-8000-000000000013",
    kind: "heartbeat",
    state: "online",
    severity: null,
    title: "Service reported online",
    message: null,
    observed_at: "2026-09-28T00:20:00Z",
    received_at: "2026-09-28T00:20:01Z",
  },
  {
    idempotency_key: "00000000-0000-4000-8000-000000000012",
    kind: "heartbeat",
    state: "offline",
    severity: null,
    title: "Service reported offline",
    message: "game process exited",
    observed_at: "2026-09-28T00:10:00Z",
    received_at: "2026-09-28T00:10:01Z",
  },
  {
    idempotency_key: "00000000-0000-4000-8000-000000000014",
    kind: "event",
    state: null,
    severity: "error",
    title: "process crash",
    message: "exit code 137",
    observed_at: "2026-09-28T00:09:00Z",
    received_at: "2026-09-28T00:09:01Z",
  },
  {
    idempotency_key: "00000000-0000-4000-8000-000000000011",
    kind: "heartbeat",
    state: "online",
    severity: null,
    title: "Service reported online",
    message: null,
    observed_at: "2026-09-27T23:50:00Z",
    received_at: "2026-09-27T23:50:01Z",
  },
];

describe("AvailabilityBoard", () => {
  afterEach(cleanup);

  it("opens a down report with its reported reason and recovery observation", () => {
    render(<AvailabilityBoard service={service} timeline={timeline} />);
    expect(screen.getByText("Looking good — service is online")).toBeTruthy();
    expect(screen.getByText("1 in recent history")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Down reported/ }));
    expect(screen.getByText(/Reported reason:/)).toBeTruthy();
    expect(screen.getByText(/game process exited/)).toBeTruthy();
    expect(screen.getByText(/First improved report/)).toBeTruthy();
    expect(screen.getByText(/Related event:/)).toBeTruthy();
    expect(screen.getByText(/exit code 137/)).toBeTruthy();
  });

  it("opens a UTC day and keeps days without observations distinct from online days", () => {
    render(<AvailabilityBoard service={service} timeline={timeline} />);
    fireEvent.click(screen.getByRole("button", { name: /2026-09-28: offline reported/ }));
    expect(screen.getByRole("heading", { name: "2026-09-28" })).toBeTruthy();
    expect(screen.getByText(/game process exited/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /2026-09-26: no heartbeat observation/ }));
    expect(
      screen.getByText("No events were recorded for this day in the loaded history."),
    ).toBeTruthy();
  });
});
