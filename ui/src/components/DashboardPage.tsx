import type { ClientSnapshot } from "../api";
import { WorkspaceSummary } from "./WorkspaceSummary";
import { useTranslation } from "../i18n";

type DashboardPageProps = {
  snapshot: ClientSnapshot;
  onProviderChange?: (workspace: string, providerId: string) => void;
};

export function DashboardPage({
  snapshot,
  onProviderChange,
}: DashboardPageProps) {
  const t = useTranslation();
  const workspaceCount = snapshot.workspaces.length;
  const workspaceCountLabel = t(workspaceCount === 1 ? "activeWorkspace" : "activeWorkspaces", { count: workspaceCount });
  return (
    <section className="client-dashboard workspace-panel-region utility-section" aria-label={t("workspaceRoutes")}>
      <div className="panel overview-panel">
        <header className="dashboard-heading">
          <div>
            <div className="dashboard-heading-title">
              <h1>{t("workspaces")}</h1>
              <span className="count dashboard-count" aria-label={workspaceCountLabel}>{workspaceCount}</span>
            </div>
            <p>{t("workspacesDescription")}</p>
          </div>
        </header>
        <div className="dashboard-grid">
          <WorkspaceSummary
            workspaces={snapshot.workspaces}
            providers={snapshot.providers}
            defaultProvider={snapshot.provider}
            onProviderChange={onProviderChange}
          />
        </div>
      </div>
    </section>
  );
}
