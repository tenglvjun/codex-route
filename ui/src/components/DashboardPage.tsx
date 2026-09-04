import { Activity } from "lucide-react";
import type { ClientSnapshot } from "../api";
import { QuickProviderSwitch } from "./QuickProviderSwitch";
import { RuntimeCard } from "./RuntimeCard";
import { WorkspaceSummary } from "./WorkspaceSummary";

type DashboardPageProps = {
  snapshot: ClientSnapshot;
  onProviderChange?: (providerId: string) => void;
  onStartRuntime?: () => void;
  onStopRuntime?: () => void;
  onOpenDiagnostics?: () => void;
};

export function DashboardPage({
  snapshot,
  onProviderChange,
  onStartRuntime,
  onStopRuntime,
  onOpenDiagnostics,
}: DashboardPageProps) {
  const workspace = snapshot.workspace;
  const providerId = workspace?.providerId || snapshot.provider?.id || "";

  return (
    <section className="client-dashboard" aria-labelledby="dashboard-heading">
      <div className="dashboard-heading">
        <div>
          <p className="eyebrow">CURRENT WORKSPACE</p>
          <h2 id="dashboard-heading">
            {workspace?.path || "No Codex workspace detected"}
          </h2>
          <p className="dashboard-lead">
            {workspace
              ? `${workspace.threadIds.length} thread${workspace.threadIds.length === 1 ? "" : "s"} in this project.`
              : "Open a Codex project to let Codex Route bind a provider automatically."}
          </p>
        </div>
        <span className={`dashboard-state dashboard-state--${snapshot.runtime.phase}`} role="status">
          <Activity size={15} aria-hidden="true" />
          {snapshot.runtime.phase.replaceAll("_", " ")}
        </span>
      </div>

      <div className="dashboard-grid">
        <WorkspaceSummary workspace={workspace} />
        <RuntimeCard snapshot={snapshot} onStart={onStartRuntime} onStop={onStopRuntime} onOpenDiagnostics={onOpenDiagnostics} />
        <QuickProviderSwitch providers={snapshot.providers} providerId={providerId} onChange={onProviderChange} />
      </div>
    </section>
  );
}
