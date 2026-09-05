import { FolderOpen, GitBranch } from "lucide-react";
import type { ProviderSummary, WorkspaceSnapshot } from "../api";
import { ProviderSelect } from "./ProviderSelect";
import { useTranslation } from "../i18n";

type WorkspaceSummaryProps = {
  workspaces: WorkspaceSnapshot[];
  providers: ProviderSummary[];
  defaultProvider?: ProviderSummary;
  onProviderChange?: (workspace: string, providerId: string) => void;
};

function workspaceName(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  return trimmed.split(/[\\/]/).pop() || trimmed || path;
}

export function WorkspaceSummary({
  workspaces,
  providers,
  defaultProvider,
  onProviderChange,
}: WorkspaceSummaryProps) {
  const t = useTranslation();
  if (workspaces.length === 0) {
    return (
      <div className="workspace-route-empty" role="status">
        <FolderOpen size={22} aria-hidden="true" />
        <div>
          <strong>{t("noActiveWorkspaces")}</strong>
          <span>{t("newSessionsUse", { provider: defaultProvider?.name || t("defaultRoute") })}</span>
        </div>
      </div>
    );
  }

  return (
    <div className="workspace-route-list" role="list" aria-label={t("workspaceRoutes")}>
      {workspaces.map((workspace) => {
        const selectedProviderId = workspace.providerId || "";
        const sessionCount = workspace.sessionIds.length;
        const sessionLabel = `${sessionCount} ${t(sessionCount === 1 ? "session" : "sessions")}`;
        return (
          <article className="workspace-route-row" role="listitem" key={workspace.path}>
            <div className="workspace-route-icon" aria-hidden="true">
              {workspace.conflictingWorkspaces ? <GitBranch size={18} /> : <FolderOpen size={18} />}
            </div>
            <div className="workspace-route-copy">
              <h2 className="workspace-route-title" title={workspace.path}>
                {workspaceName(workspace.path)} <span className="workspace-session-count" aria-label={sessionLabel}>({sessionCount})</span>
              </h2>
              <span className="workspace-route-path" title={workspace.path}>{workspace.path}</span>
              {(!workspace.exists || workspace.conflictingWorkspaces) && <div className="workspace-route-meta">
                {!workspace.exists && <span className="workspace-route-state">{t("folderUnavailable")}</span>}
                {workspace.conflictingWorkspaces && <span className="workspace-route-state">{t("ambiguousSessionMetadata")}</span>}
              </div>}
            </div>
            <ProviderSelect
              workspacePath={workspace.path}
              providers={providers}
              defaultProvider={defaultProvider}
              selectedProviderId={selectedProviderId}
              ariaLabel={t("selectProviderRoute")}
              allowEmptyOption
              className="workspace-route-select"
              onChange={(providerId) => onProviderChange?.(workspace.path, providerId)}
            />
          </article>
        );
      })}
    </div>
  );
}
