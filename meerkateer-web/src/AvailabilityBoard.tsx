import { useMemo, useState } from "react";
import type { ServiceResponse, TimelineItemResponse } from "./api";
import "./availability.css";

type Incident = {
  start: TimelineItemResponse;
  recovery: TimelineItemResponse | null;
  related: TimelineItemResponse | null;
};
type Selection =
  | { kind: "incident"; id: string }
  | { kind: "observation"; id: string }
  | { kind: "day"; date: string }
  | null;

const dateKey = (value: string) => value.slice(0, 10);
const time = (value: string) =>
  new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(value),
  );

function incidentsFrom(items: TimelineItemResponse[]): Incident[] {
  const heartbeats = items
    .filter((item) => item.kind === "heartbeat")
    .sort((a, b) => Date.parse(a.observed_at) - Date.parse(b.observed_at));
  const result: Incident[] = [];
  let start: TimelineItemResponse | null = null;
  const incident = (
    first: TimelineItemResponse,
    recovery: TimelineItemResponse | null,
  ): Incident => {
    const related = items
      .filter(
        (item) =>
          item.kind === "event" &&
          Math.abs(Date.parse(item.observed_at) - Date.parse(first.observed_at)) <= 15 * 60 * 1000,
      )
      .sort(
        (a, b) =>
          Math.abs(Date.parse(a.observed_at) - Date.parse(first.observed_at)) -
          Math.abs(Date.parse(b.observed_at) - Date.parse(first.observed_at)),
      )[0];
    return { start: first, recovery, related: related ?? null };
  };
  for (const heartbeat of heartbeats) {
    if (heartbeat.state === "offline") {
      start ??= heartbeat;
    } else if (start && (heartbeat.state === "online" || heartbeat.state === "degraded")) {
      result.push(incident(start, heartbeat));
      start = null;
    }
  }
  if (start) result.push(incident(start, null));
  return result.reverse();
}

function stateForDay(items: TimelineItemResponse[]): string {
  const states = items.filter((item) => item.kind === "heartbeat").map((item) => item.state);
  if (states.includes("offline")) return "offline";
  if (states.includes("degraded")) return "degraded";
  if (states.includes("online")) return "online";
  return "unknown";
}

export function AvailabilityBoard({
  service,
  timeline,
}: {
  service: ServiceResponse;
  timeline: TimelineItemResponse[];
}) {
  const [selection, setSelection] = useState<Selection>(null);
  const newest = timeline[0]?.observed_at ?? new Date().toISOString();
  const [monthOffset, setMonthOffset] = useState(0);
  const base = new Date(newest);
  const month = new Date(Date.UTC(base.getUTCFullYear(), base.getUTCMonth() + monthOffset, 1));
  const monthLabel = new Intl.DateTimeFormat(undefined, {
    month: "long",
    year: "numeric",
    timeZone: "UTC",
  }).format(month);
  const firstWeekday = (month.getUTCDay() + 6) % 7;
  const daysInMonth = new Date(
    Date.UTC(month.getUTCFullYear(), month.getUTCMonth() + 1, 0),
  ).getUTCDate();
  const incidents = useMemo(() => incidentsFrom(timeline), [timeline]);
  const recentHeartbeats = useMemo(
    () =>
      timeline
        .filter((item) => item.kind === "heartbeat")
        .slice(0, 42)
        .reverse(),
    [timeline],
  );
  const selectedIncident =
    selection?.kind === "incident"
      ? incidents.find((incident) => incident.start.idempotency_key === selection.id)
      : null;
  const selectedObservation =
    selection?.kind === "observation"
      ? timeline.find((item) => item.idempotency_key === selection.id)
      : null;
  const selectedDay =
    selection?.kind === "day"
      ? timeline.filter((item) => dateKey(item.observed_at) === selection.date)
      : [];
  const state = service.status.state;

  return (
    <section className="availability-board" aria-label={`${service.slug} status history`}>
      <div className={`availability-summary state-${state}`}>
        <img src="/logo.png" alt="" />
        <div>
          <p className="eyebrow">{service.game ? "Internal heartbeat" : "System status"}</p>
          <h3>
            {state === "online"
              ? "Looking good — service is online"
              : state === "offline"
                ? "Service is reporting down"
                : state === "degraded"
                  ? "Service needs attention"
                  : service.game
                    ? "Internal game signal not connected"
                    : "Waiting for a fresh signal"}
          </h3>
          <p>
            {service.status.observed_at
              ? `Last observed ${time(service.status.observed_at)}`
              : "No heartbeat has been received yet."}
          </p>
        </div>
      </div>

      <div className="availability-section-head">
        <div>
          <h3>Recent observations</h3>
          <p>Each bar is one received heartbeat. Grey means no observation, not downtime.</p>
        </div>
        <span>{recentHeartbeats.length} shown</span>
      </div>
      <fieldset className="availability-bars" aria-label="Clickable heartbeat observations">
        {recentHeartbeats.length ? (
          recentHeartbeats.map((item) => (
            <button
              aria-label={`${item.state ?? "unknown"} observed ${time(item.observed_at)}; show details`}
              aria-pressed={
                selection?.kind === "observation" && selection.id === item.idempotency_key
              }
              className={`availability-bar state-${item.state ?? "unknown"}`}
              key={item.idempotency_key}
              onClick={() => setSelection({ kind: "observation", id: item.idempotency_key })}
              title={`${item.title} · ${time(item.observed_at)}`}
              type="button"
            />
          ))
        ) : (
          <p className="availability-empty">No heartbeat observations yet.</p>
        )}
      </fieldset>

      <div className="availability-section-head">
        <div>
          <h3>Down reports</h3>
          <p>
            Start and recovery times are the first reports we received, not exact outage boundaries.
          </p>
        </div>
        <span>{incidents.length} in recent history</span>
      </div>
      <div className="availability-incidents">
        {incidents.length ? (
          incidents.map((incident) => (
            <button
              aria-expanded={
                selection?.kind === "incident" && selection.id === incident.start.idempotency_key
              }
              className="availability-incident"
              key={incident.start.idempotency_key}
              onClick={() => setSelection({ kind: "incident", id: incident.start.idempotency_key })}
              type="button"
            >
              <span className="availability-incident-dot" aria-hidden="true" />
              <span>
                <strong>Down reported {time(incident.start.observed_at)}</strong>
                <small>
                  {incident.recovery
                    ? `Recovery reported ${time(incident.recovery.observed_at)}`
                    : "No recovery heartbeat in recent history"}
                </small>
              </span>
              <span aria-hidden="true">↗</span>
            </button>
          ))
        ) : (
          <p className="availability-empty">No down reports in the loaded observations.</p>
        )}
      </div>

      <div className="availability-section-head availability-calendar-head">
        <div>
          <h3>Observation calendar</h3>
          <p>UTC days · recent history only · tap a day for its events</p>
        </div>
        <div className="availability-month-controls">
          <button
            aria-label="Previous month"
            onClick={() => setMonthOffset(monthOffset - 1)}
            type="button"
          >
            ‹
          </button>
          <span>{monthLabel}</span>
          <button
            aria-label="Next month"
            disabled={monthOffset >= 0}
            onClick={() => setMonthOffset(monthOffset + 1)}
            type="button"
          >
            ›
          </button>
        </div>
      </div>
      <fieldset className="availability-calendar" aria-label={`${monthLabel} observation calendar`}>
        {[
          ["mon", "M"],
          ["tue", "T"],
          ["wed", "W"],
          ["thu", "T"],
          ["fri", "F"],
          ["sat", "S"],
          ["sun", "S"],
        ].map(([key, label]) => (
          <span className="availability-weekday" key={key}>
            {label}
          </span>
        ))}
        {Array.from({ length: daysInMonth }, (_, index) => {
          const day = index + 1;
          const key = `${month.getUTCFullYear()}-${String(month.getUTCMonth() + 1).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
          const dayItems = timeline.filter((item) => dateKey(item.observed_at) === key);
          const dayState = stateForDay(dayItems);
          return (
            <button
              aria-label={`${key}: ${dayState === "unknown" ? "no heartbeat observation" : `${dayState} reported`}; show day details`}
              aria-pressed={selection?.kind === "day" && selection.date === key}
              className={`availability-day state-${dayState}`}
              key={key}
              onClick={() => setSelection({ kind: "day", date: key })}
              style={day === 1 ? { gridColumnStart: firstWeekday + 1 } : undefined}
              type="button"
            >
              <span>{day}</span>
              <i aria-hidden="true" />
            </button>
          );
        })}
      </fieldset>

      {selection ? (
        <section className="availability-detail" aria-label="Selected status details">
          <button
            className="availability-close"
            onClick={() => setSelection(null)}
            type="button"
            aria-label="Close status details"
          >
            ×
          </button>
          {selectedIncident ? (
            <>
              <p className="eyebrow">Down report details</p>
              <h4>First down report: {time(selectedIncident.start.observed_at)}</h4>
              <p>
                <strong>Reported reason:</strong>{" "}
                {selectedIncident.start.message || "No reason was included with this heartbeat."}
              </p>
              <p>
                <strong>Recovery:</strong>{" "}
                {selectedIncident.recovery
                  ? `First improved report ${time(selectedIncident.recovery.observed_at)} (${selectedIncident.recovery.state}).`
                  : "No recovery heartbeat in the loaded history."}
              </p>
              {selectedIncident.related ? (
                <p>
                  <strong>Related event:</strong> {selectedIncident.related.title}
                  {selectedIncident.related.message ? ` — ${selectedIncident.related.message}` : ""}
                  . This event is context, not a confirmed root cause.
                </p>
              ) : null}
            </>
          ) : selectedObservation ? (
            <>
              <p className="eyebrow">Heartbeat detail</p>
              <h4>{selectedObservation.title}</h4>
              <p>
                Observed {time(selectedObservation.observed_at)} · received{" "}
                {time(selectedObservation.received_at)}
              </p>
              <p>
                <strong>Reported reason:</strong>{" "}
                {selectedObservation.message || "No reason was included with this heartbeat."}
              </p>
            </>
          ) : selection.kind === "day" ? (
            <>
              <p className="eyebrow">UTC day detail</p>
              <h4>{selection.date}</h4>
              {selectedDay.length ? (
                <ul>
                  {selectedDay.map((item) => (
                    <li key={item.idempotency_key}>
                      <strong>{item.title}</strong> · {time(item.observed_at)}
                      {item.message ? ` — ${item.message}` : ""}
                    </li>
                  ))}
                </ul>
              ) : (
                <p>No events were recorded for this day in the loaded history.</p>
              )}
            </>
          ) : null}
        </section>
      ) : null}
      <p className="availability-footnote">
        Showing at most 100 recent timeline events. Historical gaps and exact uptime are not
        inferred from sparse heartbeats.
      </p>
    </section>
  );
}
