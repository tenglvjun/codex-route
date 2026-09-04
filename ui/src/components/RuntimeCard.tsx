import { Wrench } from "lucide-react";
import type { ClientSnapshot } from "../api";

function runtimeCopy(snapshot: ClientSnapshot) {
  switch (snapshot.runtime.phase) {
    case "running": return "Route is running and ready for Codex requests.";
    case "starting": return "Starting the local Route runtime…";
    case "recovering": return "Route stopped unexpectedly. Attempting recovery.";
    case "blocked_external_modification": return "Codex config changed outside Codex Route.";
    case "failed": return snapshot.runtime.lastError || "Route could not be started.";
    case "degraded": return snapshot.runtime.lastError || "Route needs attention.";
    default: return "Route is not running.";
  }
}

type RuntimeCardProps = {
  snapshot: ClientSnapshot;
  onStart?: () => void;
  onStop?: () => void;
  onOpenDiagnostics?: () => void;
};

export function RuntimeCard({ snapshot, onStart, onStop, onOpenDiagnostics }: RuntimeCardProps) {
  const showDiagnostics = snapshot.diagnostics.unreadCount > 0 || ["degraded", "failed", "blocked_external_modification"].includes(snapshot.runtime.phase);
  return (
    <div className="dashboard-card dashboard-card-runtime">
      <div className="dashboard-card-icon" aria-hidden="true"><Wrench size={18} /></div>
      <div><p className="eyebrow">RUNTIME</p><strong>{runtimeCopy(snapshot)}</strong><span>{snapshot.runtime.port ? `Listening on 127.0.0.1:${snapshot.runtime.port}` : "No local listener"}</span></div>
      <div className="dashboard-card-actions">
        {snapshot.runtime.active
          ? <button className="button-secondary-pill" type="button" onClick={onStop}>Stop Route</button>
          : <button className="button-primary" type="button" onClick={onStart}>Start Route</button>}
        {showDiagnostics && <button className="text-button" type="button" onClick={onOpenDiagnostics}>Open diagnostics{snapshot.diagnostics.unreadCount > 0 ? ` (${snapshot.diagnostics.unreadCount})` : ""}</button>}
      </div>
    </div>
  );
}
