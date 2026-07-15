import type { ConnectionStatus, SnapshotMessage } from "../types";

export interface SidePanelProps {
  status: ConnectionStatus;
  statusDetail?: string;
  snapshot: SnapshotMessage | null;
}

function statusLabel(status: ConnectionStatus): string {
  switch (status) {
    case "connecting":
      return "CONNECTING";
    case "connected":
      return "ONLINE";
    case "disconnected":
      return "OFFLINE";
    case "error":
      return "ERROR";
    default:
      return status;
  }
}

/**
 * Side panel: connection, tick, focused agent, inventory, recent events.
 */
export function SidePanel({ status, statusDetail, snapshot }: SidePanelProps) {
  const focusedId = snapshot?.focusedAgentId ?? null;
  const focusedEntity =
    focusedId && snapshot
      ? snapshot.entities.find((e) => e.id === focusedId)
      : undefined;
  const inventoryOre = focusedEntity?.inventory?.ore ?? null;
  const events = (snapshot?.recentEvents ?? []).slice(-20).reverse();

  return (
    <aside className="side-panel">
      <header className="side-panel__header">
        <h1>ACO V1</h1>
        <p className="side-panel__sub">Agent Civilization OS</p>
      </header>

      <section className="side-panel__section">
        <h2>Connection</h2>
        <div className={`status status--${status}`}>
          <span className="status__dot" />
          <span className="status__label">{statusLabel(status)}</span>
        </div>
        {statusDetail ? (
          <p className="side-panel__muted">{statusDetail}</p>
        ) : null}
      </section>

      <section className="side-panel__section">
        <h2>World</h2>
        <dl className="kv">
          <div>
            <dt>Tick</dt>
            <dd>{snapshot ? snapshot.tick : "—"}</dd>
          </div>
          <div>
            <dt>Map</dt>
            <dd>
              {snapshot
                ? `${snapshot.width}×${snapshot.height}`
                : "—"}
            </dd>
          </div>
          <div>
            <dt>Entities</dt>
            <dd>{snapshot ? snapshot.entities.length : "—"}</dd>
          </div>
        </dl>
      </section>

      <section className="side-panel__section">
        <h2>Focus</h2>
        <dl className="kv">
          <div>
            <dt>Agent</dt>
            <dd className="mono">{focusedId ?? "none"}</dd>
          </div>
          <div>
            <dt>Inventory ore</dt>
            <dd>{inventoryOre !== null ? inventoryOre : "—"}</dd>
          </div>
          {focusedEntity ? (
            <div>
              <dt>Position</dt>
              <dd>
                ({focusedEntity.x}, {focusedEntity.y})
              </dd>
            </div>
          ) : null}
        </dl>
        <p className="side-panel__hint">Click an agent on the map to select.</p>
      </section>

      <section className="side-panel__section side-panel__events">
        <h2>Events</h2>
        {events.length === 0 ? (
          <p className="side-panel__muted">No recent events</p>
        ) : (
          <ul className="event-list">
            {events.map((ev, i) => (
              <li key={`${ev.tick ?? "?"}-${ev.type}-${i}`} className="event-list__item">
                <span className="event-list__tick">
                  t{ev.tick ?? "?"}
                </span>
                <span className="event-list__type">{ev.type}</span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </aside>
  );
}
