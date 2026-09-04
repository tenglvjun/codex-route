import { Wrench } from "lucide-react";
import type { ClientSnapshot } from "../api";

function runtimeCopy(snapshot: ClientSnapshot) {
  switch (snapshot.runtime.phase) {
    case "running": return "Ready";
    case "starting": return "Starting…";
    case "recovering": return "Recovering…";
    case "blocked_external_modification": return "Protected";
    case "failed": return snapshot.runtime.lastError || "Failed";
    case "degraded": return snapshot.runtime.lastError || "Needs attention";
    default: return "Stopped";
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
      <div><strong>{runtimeCopy(snapshot)}</strong><span>{snapshot.runtime.port ? `127.0.0.1:${snapshot.runtime.port}` : "No listener"}</span></div>
      <div className="dashboard-card-actions">
        {snapshot.runtime.active
          ? <button className="button-secondary-pill" type="button" onClick={onStop}>Stop Route</button>
          : <button className="button-primary" type="button" onClick={onStart}>Start Route</button>}
        {showDiagnostics && <button className="text-button" type="button" onClick={onOpenDiagnostics}>Open diagnostics{snapshot.diagnostics.unreadCount > 0 ? ` (${snapshot.diagnostics.unreadCount})` : ""}</button>}
      </div>
    </div>
  );
}
