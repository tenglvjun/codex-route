import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Activity,
  FileInput,
  FolderTree,
  Plus,
  RefreshCw,
  Server,
} from "lucide-react";
import {
  desktopApi,
  type ImportCcSwitchRequest,
  type ImportReport,
  type LifecycleStatus,
  type ProviderSummary,
  type UpsertRouteRuleRequest,
  type WorkspaceRouteRule,
} from "./api";
import { ProviderPanel } from "./components/ProviderPanel";
import { RouteStatusPanel } from "./components/RouteStatusPanel";
import { WorkspaceRulesPanel } from "./components/WorkspaceRulesPanel";
import { displayError } from "./errors";

const DEFAULT_PORT = 16729;
type ActiveView = "providers" | "rules";

function App() {
  const [providers, setProviders] = useState<ProviderSummary[]>([]);
  const [rules, setRules] = useState<WorkspaceRouteRule[]>([]);
  const [status, setStatus] = useState<LifecycleStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [port, setPort] = useState(String(DEFAULT_PORT));
  const [activeView, setActiveView] = useState<ActiveView>("providers");
  const [importOpen, setImportOpen] = useState(false);
  const refreshVersion = useRef(0);

  const currentProvider = useMemo(
    () => providers.find((provider) => provider.isCurrent),
    [providers],
  );

  const routeLabel = useMemo(() => {
    if (!status) return "Loading route";
    if (status.externalModification) return "Protected";
    if (status.active) return "Active";
    return "Inactive";
  }, [status]);

  const refresh = useCallback(async (): Promise<boolean> => {
    const version = ++refreshVersion.current;
    try {
      const [nextProviders, nextRules, nextStatus] = await Promise.all([
        desktopApi.listProviders(),
        desktopApi.listRouteRules(),
        desktopApi.getLifecycleStatus(),
      ]);
      if (version !== refreshVersion.current) return false;

      setProviders(nextProviders);
      setRules(nextRules);
      setStatus(nextStatus);
      setError(null);
      if (nextStatus.port) setPort(String(nextStatus.port));
      return true;
    } catch (cause) {
      if (version === refreshVersion.current) setError(displayError(cause));
      return false;
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const runRefreshingAction = async <Result,>(action: () => Promise<Result>): Promise<Result> => {
    setBusy(true);
    setError(null);
    try {
      const result = await action();
      await refresh();
      return result;
    } finally {
      setBusy(false);
    }
  };

  const runAction = async (action: () => Promise<unknown>): Promise<boolean> => {
    try {
      await runRefreshingAction(action);
      return true;
    } catch (cause) {
      setError(displayError(cause));
      return false;
    }
  };

  const refreshManually = async () => {
    setBusy(true);
    setError(null);
    try {
      await refresh();
    } finally {
      setBusy(false);
    }
  };

  const saveRule = async (request: UpsertRouteRuleRequest): Promise<void> => {
    await runRefreshingAction(() => desktopApi.upsertRouteRule(request));
  };

  const importProviders = (request: ImportCcSwitchRequest): Promise<ImportReport> =>
    runRefreshingAction(() => desktopApi.importCcSwitchProviders(request));

  const activate = async () => {
    const numericPort = Number(port);
    if (!Number.isInteger(numericPort) || numericPort < 1 || numericPort > 65535) {
      setError("Port must be an integer between 1 and 65535.");
      return;
    }
    await runAction(() => desktopApi.activateRoute(numericPort));
  };

  const toggleRoute = () => {
    if (!status || busy) return;
    if (status.active) {
      if (status.externalModification) return;
      void runAction(() => desktopApi.deactivateRoute());
      return;
    }
    void activate();
  };

  const changeView = (view: ActiveView) => {
    setActiveView(view);
    if (view !== "providers") setImportOpen(false);
  };

  const statusModifier = status?.externalModification
    ? " protected"
    : status?.active
      ? " active"
      : status
        ? " inactive"
        : " loading";
  const routeState = status?.externalModification
    ? "external-modified"
    : status?.active
      ? "active"
      : status
        ? "inactive"
        : "loading";

  return (
    <div className="client-window">
      <header className="client-toolbar" aria-label="Codex Route toolbar">
        <div className="toolbar-brand">
          <span className="brand-mark" aria-hidden="true">CR</span>
          <span className="toolbar-brand-copy">
            <strong>Codex Route</strong>
            <small>Provider switcher</small>
          </span>
        </div>

        <nav className="toolbar-view-switch" aria-label="Workspace view" role="tablist">
          <button
            className={`view-tab${activeView === "providers" ? " active" : ""}`}
            type="button"
            role="tab"
            aria-selected={activeView === "providers"}
            onClick={() => changeView("providers")}
          >
            <Server size={16} aria-hidden="true" />
            Providers
          </button>
          <button
            className={`view-tab${activeView === "rules" ? " active" : ""}`}
            type="button"
            role="tab"
            aria-selected={activeView === "rules"}
            onClick={() => changeView("rules")}
          >
            <FolderTree size={16} aria-hidden="true" />
            Workspace rules
          </button>
        </nav>

        <div className="toolbar-route">
          <span className={`route-context${statusModifier}`} role="status" aria-live="polite">
            <Activity size={15} aria-hidden="true" />
            <span>{routeLabel}</span>
          </span>
          <button
            className={`route-toggle${status?.active ? " active" : ""}`}
            type="button"
            role="switch"
            aria-checked={status?.active === true}
            data-route-state={routeState}
            aria-label={status?.active ? "Deactivate route" : "Activate route"}
            onClick={toggleRoute}
            disabled={
              busy ||
              !status ||
              (!status.active && currentProvider === undefined) ||
              status.externalModification
            }
          >
            <span className="route-toggle-track" aria-hidden="true"><span /></span>
            <span className="route-toggle-label">Route</span>
          </button>
        </div>

        <div className="toolbar-actions">
          <button
            className="icon-button toolbar-action"
            type="button"
            aria-label="Refresh"
            title="Refresh"
            onClick={() => void refreshManually()}
            disabled={busy}
          >
            <RefreshCw className={busy ? "spin" : undefined} size={17} aria-hidden="true" />
          </button>
          {activeView === "providers" && (
            <button
              className="round-action"
              type="button"
              aria-label="Import providers"
              title="Import providers"
              onClick={() => setImportOpen(true)}
              disabled={busy}
            >
              <FileInput size={18} aria-hidden="true" />
            </button>
          )}
          <button
            className="round-action primary"
            type="button"
            aria-label="Add workspace rule"
            title="Add workspace rule"
            onClick={() => changeView("rules")}
            disabled={busy}
          >
            <Plus size={19} aria-hidden="true" />
          </button>
        </div>
      </header>

      <main className="workspace-frame">
        <header className="workspace-heading">
          <div>
            <p className="eyebrow">LOCAL WORKSPACE</p>
            <h1>{activeView === "providers" ? "Providers" : "Workspace rules"}</h1>
            <p className="subtitle">
              {activeView === "providers"
                ? "Choose the provider Codex should use for local requests."
                : "Route each Codex workspace to its preferred provider."}
            </p>
          </div>
          <div className="workspace-meta" aria-label="Workspace summary">
            <span>{providers.length} provider{providers.length === 1 ? "" : "s"}</span>
            <span>{rules.length} rule{rules.length === 1 ? "" : "s"}</span>
            {currentProvider && <span className="current-provider">Using {currentProvider.name}</span>}
          </div>
        </header>

        {error && <div className="error" role="alert">{error}</div>}

        <div className="workspace-content">
          {activeView === "providers" ? (
            <>
              <section className="route-panel-region" aria-label="Route configuration">
                <RouteStatusPanel
                  status={status}
                  port={port}
                  busy={busy}
                  canActivate={currentProvider !== undefined}
                  onPortChange={setPort}
                  onActivate={() => void activate()}
                  onDeactivate={() => void runAction(() => desktopApi.deactivateRoute())}
                />
              </section>
              <section className="workspace-panel-region" aria-labelledby="providers-heading">
                <ProviderPanel
                  providers={providers}
                  busy={busy}
                  onSelect={(providerId) => void runAction(() => desktopApi.setCurrentProvider(providerId))}
                  onImport={importProviders}
                  onError={setError}
                  importOpen={importOpen}
                  onImportOpenChange={setImportOpen}
                />
              </section>
            </>
          ) : (
            <section className="workspace-panel-region" aria-labelledby="rules-heading">
              <WorkspaceRulesPanel
                providers={providers}
                rules={rules}
                busy={busy}
                onSave={saveRule}
                onRemove={(workspace) => runAction(() => desktopApi.removeRouteRule(workspace))}
                onError={setError}
                onClose={() => changeView("providers")}
              />
            </section>
          )}
        </div>
      </main>
    </div>
  );
}

export default App;
