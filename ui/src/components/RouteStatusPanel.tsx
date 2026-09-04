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
  const routeState = status?.externalModification
    ? "external-modified"
    : status?.active
      ? "active"
      : status
        ? "inactive"
        : "loading";
  const legacyStatusModifier = status?.externalModification
    ? "external"
    : status?.active
      ? "active"
      : status
        ? "inactive"
        : "loading";
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
    <section
      className={`route-status-card route-control-strip route-control-strip--${routeState}`}
      data-route-state={routeState}
      data-state={routeState}
      aria-labelledby="status-heading"
    >
      <div className="route-strip-head">
        <div className="status-heading-block route-strip-identity">
          <span
            className={`route-status-indicator route-status-indicator--${routeState}`}
            aria-hidden="true"
          />
          <div>
            <p className="eyebrow">LOCAL ROUTE</p>
            <h2 id="status-heading">Route status</h2>
            <p className="muted">Local listener for Codex provider routing.</p>
          </div>
        </div>

        <div
          className={`status-state route-strip-status ${legacyStatusModifier} route-strip-status--${routeState}`}
          data-route-state={routeState}
          role="status"
          aria-live="polite"
        >
          <span className="status-state-dot" aria-hidden="true" />
          <span>{statusLabel}</span>
        </div>
      </div>

      <div className="status-details route-strip-details" aria-label="Route details">
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
        <p className="status-warning route-strip-warning" role="alert">
          Codex config changed outside Codex Route. Deactivation is blocked to protect it.
        </p>
      )}

      <div
        className="status-actions route-strip-controls"
        data-route-state={routeState}
      >
        <label className="compact-field route-port-field" htmlFor="route-port">
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
        <div className="actions route-switch" data-route-state={routeState} role="group" aria-label="Route controls">
          <button
            className="button primary route-switch-action route-switch-action--activate"
            type="button"
            data-route-action="activate"
            onClick={onActivate}
            disabled={busy || status?.active === true || !canActivate}
          >
            <Power size={16} aria-hidden="true" />
            Activate
          </button>
          <button
            className="button danger route-switch-action route-switch-action--deactivate"
            type="button"
            data-route-action="deactivate"
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
