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
  return (
    <section className="status-card" aria-labelledby="status-heading">
      <div className="status-summary">
        <p className="eyebrow" id="status-heading">ROUTE STATUS</p>
        <strong
          className={`status${status?.active ? " active" : ""}${status?.externalModification ? " external" : ""}`}
          role="status"
          aria-live="polite"
        >
          {status?.status ?? "loading"}
        </strong>
        <p className="muted">
          {status?.port ? `127.0.0.1:${status.port}` : "No listener"}
          {status?.configManaged ? " · Codex config managed" : ""}
        </p>
        {status?.active && status.port && (
          <p className="route-url">http://127.0.0.1:{status.port}/v1</p>
        )}
        {status?.externalModification && (
          <p className="status-warning">
            Codex config changed outside Codex Route. Deactivation is blocked to protect it.
          </p>
        )}
      </div>
      <div className="route-controls">
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
