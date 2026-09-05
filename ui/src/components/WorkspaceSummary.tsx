import { FolderOpen, GitBranch } from "lucide-react";
import type { ProviderSummary, WorkspaceSnapshot } from "../api";

type WorkspaceSummaryProps = {
  workspaces: WorkspaceSnapshot[];
  providers: ProviderSummary[];
  defaultProvider?: ProviderSummary;
  onProviderChange?: (workspace: string, providerId: string) => void;
};

export function WorkspaceSummary({
  workspaces,
  providers,
  defaultProvider,
  onProviderChange,
}: WorkspaceSummaryProps) {
  if (workspaces.length === 0) {
    return (
      <div className="workspace-route-empty" role="status">
        <FolderOpen size={22} aria-hidden="true" />
        <div>
          <strong>No active Codex workspaces</strong>
          <span>New sessions will use {defaultProvider ? `${defaultProvider.name} by default` : "the default route"}.</span>
        </div>
      </div>
    );
  }

  return (
    <div className="workspace-route-list" aria-label="Workspace routes">
      <div className="workspace-route-list-header" aria-hidden="true">
        <span>Active workspaces</span>
        <span>Provider route</span>
      </div>
      {workspaces.map((workspace) => {
        const selectedProviderId = workspace.providerId || "";
        return (
          <div className="workspace-route-row" key={workspace.path}>
            <div className="workspace-route-icon" aria-hidden="true">
              {workspace.conflictingWorkspaces ? <GitBranch size={18} /> : <FolderOpen size={18} />}
            </div>
            <div className="workspace-route-copy">
              <strong title={workspace.path}>{workspace.path}</strong>
              <span>
                {workspace.sessionIds.length} session{workspace.sessionIds.length === 1 ? "" : "s"} · {workspace.threadIds.length} thread{workspace.threadIds.length === 1 ? "" : "s"}
                {!workspace.exists ? " · folder unavailable" : ""}
                {workspace.conflictingWorkspaces ? " · ambiguous session metadata" : ""}
              </span>
            </div>
            <label className="workspace-route-select">
              <span className="sr-only">Route for {workspace.path}</span>
              <select
                value={selectedProviderId}
                onChange={(event) => onProviderChange?.(workspace.path, event.target.value)}
                disabled={providers.length === 0 || !onProviderChange}
              >
                <option value="">Use default{defaultProvider ? ` · ${defaultProvider.name}` : " route"}</option>
                {providers.map((provider) => (
                  <option value={provider.id} key={provider.id}>{provider.name}</option>
                ))}
              </select>
            </label>
          </div>
        );
      })}
    </div>
  );
}
