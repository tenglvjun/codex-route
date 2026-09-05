import { Wrench } from "lucide-react";
import type { ClientSnapshot } from "../api";
import { useTranslation } from "../i18n";

function runtimeCopy(snapshot: ClientSnapshot, t: ReturnType<typeof useTranslation>) {
  switch (snapshot.runtime.phase) {
    case "running": return t("ready");
    case "starting": return t("starting");
    case "recovering": return t("recovering");
    case "blocked_external_modification": return t("protected");
    case "failed": return snapshot.runtime.lastError || t("failed");
    case "degraded": return snapshot.runtime.lastError || t("needsAttention");
    default: return t("stopped");
  }
}

type RuntimeCardProps = {
  snapshot: ClientSnapshot;
  onStart?: () => void;
  onStop?: () => void;
  onOpenDiagnostics?: () => void;
};

export function RuntimeCard({ snapshot, onStart, onStop, onOpenDiagnostics }: RuntimeCardProps) {
  const t = useTranslation();
  const showDiagnostics = snapshot.diagnostics.unreadCount > 0 || ["degraded", "failed", "blocked_external_modification"].includes(snapshot.runtime.phase);
  return (
    <div className="dashboard-card dashboard-card-runtime">
      <div className="dashboard-card-icon" aria-hidden="true"><Wrench size={18} /></div>
      <div><strong>{runtimeCopy(snapshot, t)}</strong><span>{snapshot.runtime.port ? `127.0.0.1:${snapshot.runtime.port}` : t("noListener")}</span></div>
      <div className="dashboard-card-actions">
        {snapshot.runtime.active
          ? <button className="button-secondary-pill" type="button" onClick={onStop}>{t("stopRouteAction")}</button>
          : <button className="button-primary" type="button" onClick={onStart}>{t("startRouteAction")}</button>}
        {showDiagnostics && <button className="text-button" type="button" onClick={onOpenDiagnostics}>{t("openDiagnostics")}{snapshot.diagnostics.unreadCount > 0 ? ` (${snapshot.diagnostics.unreadCount})` : ""}</button>}
      </div>
    </div>
  );
}
