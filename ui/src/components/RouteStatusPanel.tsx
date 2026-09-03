import { Power, Square } from "lucide-react";
import type { LifecycleStatus } from "../api";

type RouteStatusPanelProps = {
  status: LifecycleStatus | null;
  port: string;
  busy: boolean;
  canActivate: boolean;
  onPortChange: (port: string) => void;
  onActivate: () => void;
  onDeactivate: () => void;
};

export function RouteStatusPanel({
  status,
  port,
  busy,
  canActivate,
  onPortChange,
  onActivate,
  onDeactivate,
}: RouteStatusPanelProps) {
  const statusModifier = status?.externalModification
    ? " external"
    : status?.active
      ? " active"
      : status
        ? " inactive"
        : " loading";
  const statusLabel = status?.externalModification
    ? "External modification"
    : status?.active
      ? "Active"
      : status
        ? "Inactive"
        : "Loading";
  const listener = status?.port ? `127.0.0.1:${status.port}` : "No listener";
  const routeUrl = status?.active && status.port
    ? `http://127.0.0.1:${status.port}/v1`
    : "Unavailable";

  return (
    <section className="route-status-card" aria-labelledby="status-heading">
      <div className="status-heading-block">
        <p className="eyebrow">ROUTE STATUS</p>
        <h2 id="status-heading">Route status</h2>
        <p className="muted">Manage the local listener used for Codex provider routing.</p>
      </div>

      <div className={`status-state${statusModifier}`} role="status" aria-live="polite">
        <span className="status-state-dot" aria-hidden="true" />
        <span>{statusLabel}</span>
      </div>

      <div className="status-details" aria-label="Route details">
        <div className="status-detail">
          <span className="muted">Listener</span>
          <strong className="status-value">{listener}</strong>
        </div>
        <div className="status-detail">
          <span className="muted">Configuration</span>
          <strong className="status-value">
            {status?.configManaged ? "Managed by Codex Route" : "Not managed"}
          </strong>
        </div>
        <div className="status-detail">
          <span className="muted">Route URL</span>
          <code className="route-url">{routeUrl}</code>
        </div>
      </div>

      {status?.externalModification && (
        <p className="status-warning" role="alert">
          Codex config changed outside Codex Route. Deactivation is blocked to protect it.
        </p>
      )}

      <div className="status-actions">
        <label className="compact-field" htmlFor="route-port">
          <span>Port</span>
          <input
            id="route-port"
            type="number"
            min="1"
            max="65535"
            value={port}
            onChange={(event) => onPortChange(event.target.value)}
            disabled={busy || status?.active === true}
          />
        </label>
        <div className="actions">
          <button
            className="button primary"
            onClick={onActivate}
            disabled={busy || status?.active === true || !canActivate}
          >
            <Power size={16} aria-hidden="true" />
            Activate
          </button>
          <button
            className="button danger"
            onClick={onDeactivate}
            disabled={busy || status?.active !== true}
          >
            <Square size={15} aria-hidden="true" />
            Deactivate
          </button>
        </div>
      </div>
    </section>
  );
}
