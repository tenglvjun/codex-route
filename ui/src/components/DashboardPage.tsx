import { FolderTree } from "lucide-react";
import type { ClientSnapshot } from "../api";
import { RuntimeCard } from "./RuntimeCard";
import { WorkspaceSummary } from "./WorkspaceSummary";

type DashboardPageProps = {
  snapshot: ClientSnapshot;
  onProviderChange?: (workspace: string, providerId: string) => void;
  onStartRuntime?: () => void;
  onStopRuntime?: () => void;
  onOpenDiagnostics?: () => void;
  workspaceRulesOpen: boolean;
  onToggleWorkspaceRules: () => void;
};

export function DashboardPage({
  snapshot,
  onProviderChange,
  onStartRuntime,
  onStopRuntime,
  onOpenDiagnostics,
  workspaceRulesOpen,
  onToggleWorkspaceRules,
}: DashboardPageProps) {
  return (
    <section className="client-dashboard" aria-labelledby="dashboard-heading">
      <div className="dashboard-heading">
        <div>
          <p className="eyebrow">OVERVIEW</p>
          <h2 id="dashboard-heading">Workspace routes</h2>
          <p className="dashboard-lead">
            {snapshot.workspaces.length} active workspace{snapshot.workspaces.length === 1 ? "" : "s"} · new sessions use {snapshot.provider?.name || "the default route"}
          </p>
        </div>
        <div className="dashboard-heading-actions">
          <button
            className="button-secondary-pill"
            type="button"
            aria-haspopup="dialog"
            aria-expanded={workspaceRulesOpen}
            aria-controls="workspace-rules-dialog"
            onClick={onToggleWorkspaceRules}
          >
            <FolderTree size={16} aria-hidden="true" />
            {workspaceRulesOpen ? "Hide route settings" : "Configure routes"}
          </button>
        </div>
      </div>

      <div className="dashboard-grid">
        <WorkspaceSummary
          workspaces={snapshot.workspaces}
          providers={snapshot.providers}
          defaultProvider={snapshot.provider}
          onProviderChange={onProviderChange}
        />
        <RuntimeCard snapshot={snapshot} onStart={onStartRuntime} onStop={onStopRuntime} onOpenDiagnostics={onOpenDiagnostics} />
      </div>
    </section>
  );
}
