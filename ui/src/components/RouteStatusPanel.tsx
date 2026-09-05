import { Power, Square } from "lucide-react";
import type { LifecycleStatus } from "../api";
import { useTranslation } from "../i18n";

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
  const t = useTranslation();
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
    ? t("externalModification")
    : status?.active
      ? t("active")
      : status
        ? t("inactive")
        : t("loadingRoute");
  const listener = status?.port ? `127.0.0.1:${status.port}` : t("noListener");
  const routeUrl = status?.active && status.port
    ? `http://127.0.0.1:${status.port}/v1`
    : t("unavailable");

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
            <p className="eyebrow">{t("localRoute")}</p>
            <h2 id="status-heading">{t("routeStatus")}</h2>
            <p className="muted">{t("localListenerDescription")}</p>
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

      <div className="status-details route-strip-details" aria-label={t("routeStatus")}>
        <div className="status-detail">
          <span className="muted">{t("listener")}</span>
          <strong className="status-value">{listener}</strong>
        </div>
        <div className="status-detail">
          <span className="muted">{t("configuration")}</span>
          <strong className="status-value">
            {status?.configManaged ? t("managedByCodexRoute") : t("notManaged")}
          </strong>
        </div>
        <div className="status-detail">
          <span className="muted">{t("routeUrl")}</span>
          <code className="route-url">{routeUrl}</code>
        </div>
      </div>

      {status?.externalModification && (
        <p className="status-warning route-strip-warning" role="alert">
          {t("externalConfigWarning")}
        </p>
      )}

      <div
        className="status-actions route-strip-controls"
        data-route-state={routeState}
      >
        <label className="compact-field route-port-field" htmlFor="route-port">
          <span>{t("port")}</span>
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
        <div className="actions route-switch" data-route-state={routeState} role="group" aria-label={t("routeControls")}>
          <button
            className="button primary route-switch-action route-switch-action--activate"
            type="button"
            data-route-action="activate"
            onClick={onActivate}
            disabled={busy || status?.active === true || !canActivate}
          >
            <Power size={16} aria-hidden="true" />
            {t("activate")}
          </button>
          <button
            className="button danger route-switch-action route-switch-action--deactivate"
            type="button"
            data-route-action="deactivate"
            onClick={onDeactivate}
            disabled={busy || status?.active !== true}
          >
            <Square size={15} aria-hidden="true" />
            {t("deactivate")}
          </button>
        </div>
      </div>
    </section>
  );
}
