import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Activity,
  LayoutDashboard,
  Menu,
  RefreshCw,
  Server,
  Settings,
  X,
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
import { WorkspaceRulesPanel } from "./components/WorkspaceRulesPanel";
import { displayError } from "./errors";
import { clientFacade } from "./clientFacade";
import type { ClientSnapshot, DiagnosticRecord } from "./api";
import { DashboardPage } from "./components/DashboardPage";
import { DiagnosticsPanel } from "./components/DiagnosticsPanel";
import { SettingsPanel } from "./components/SettingsPanel";

const DEFAULT_PORT = 16729;
type ActiveView = "dashboard" | "providers" | "settings";

function App() {
  const [providers, setProviders] = useState<ProviderSummary[]>([]);
  const [rules, setRules] = useState<WorkspaceRouteRule[]>([]);
  const [status, setStatus] = useState<LifecycleStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [port, setPort] = useState(String(DEFAULT_PORT));
  const [activeView, setActiveView] = useState<ActiveView>("dashboard");
  const [importOpen, setImportOpen] = useState(false);
  const [workspaceRulesOpen, setWorkspaceRulesOpen] = useState(false);
  const [mobileNavOpen, setMobileNavOpen] = useState(false);
  const [clientSnapshot, setClientSnapshot] = useState<ClientSnapshot | null>(null);
  const [diagnostics, setDiagnostics] = useState<DiagnosticRecord[]>([]);
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false);

  const applySnapshot = useCallback((snapshot: ClientSnapshot) => {
    setClientSnapshot(snapshot);
    setProviders(snapshot.providers);
    setRules(snapshot.rules);
    setStatus({
      status: snapshot.runtime.phase,
      active: snapshot.runtime.active,
      pid: snapshot.runtime.pid,
      port: snapshot.runtime.port,
      serverReachable: snapshot.runtime.serverReachable,
      configManaged: snapshot.runtime.configManaged,
      externalModification: snapshot.runtime.externalModification,
      configPath: snapshot.codex.configPath,
      statePath: "",
      lockPath: "",
    });
    setPort(snapshot.runtime.port ? String(snapshot.runtime.port) : String(DEFAULT_PORT));
  }, []);

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
    setLoading(true);
    try {
      const snapshot = await clientFacade.loadSnapshot();
      applySnapshot(snapshot);
      setError(null);
      return true;
    } catch (cause) {
      setError(displayError(cause));
      return false;
    } finally {
      setLoading(false);
    }
  }, [applySnapshot]);

  useEffect(() => {
    let cancelled = false;
    const unsubscriptions: Array<() => void> = [];
    const keepSubscription = (cancel: () => void) => {
      if (cancelled) cancel();
      else unsubscriptions.push(cancel);
    };
    void clientFacade.getDiagnostics(20).then(setDiagnostics).catch(() => undefined);
    void clientFacade.subscribe((snapshot) => {
      applySnapshot(snapshot);
    }).then(keepSubscription).catch(() => undefined);
    void clientFacade.subscribeDiagnostics((record) => {
      setDiagnostics((current) => [record, ...current.filter((item) => item.id !== record.id)].slice(0, 20));
    }).then(keepSubscription).catch(() => undefined);
    void refresh();
    return () => {
      cancelled = true;
      unsubscriptions.forEach((unsubscribe) => unsubscribe());
    };
  }, [applySnapshot, refresh]);

  const runBusyAction = async <Result,>(action: () => Promise<Result>): Promise<Result> => {
    setBusy(true);
    setError(null);
    try {
      return await action();
    } finally {
      setBusy(false);
    }
  };

  const runAction = async (action: () => Promise<unknown>): Promise<boolean> => {
    try {
      await runBusyAction(action);
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
    await runBusyAction(() => desktopApi.upsertRouteRule(request));
  };

  const importProviders = (request: ImportCcSwitchRequest): Promise<ImportReport> =>
    runBusyAction(() => desktopApi.importCcSwitchProviders(request));

  const changeWorkspaceProvider = async (workspace: string, providerId: string) => {
    setBusy(true);
    setError(null);
    try {
      if (!providerId) {
        await desktopApi.removeRouteRule(workspace);
        await refresh();
        return;
      }
      const snapshot = await clientFacade.setWorkspaceProvider(workspace, providerId);
      applySnapshot(snapshot);
    } catch (cause) {
      setError(displayError(cause));
    } finally {
      setBusy(false);
    }
  };

  const startRuntime = async () => {
    setBusy(true);
    setError(null);
    try {
      const snapshot = await clientFacade.startRuntime();
      applySnapshot(snapshot);
    } catch (cause) {
      setError(displayError(cause));
    } finally {
      setBusy(false);
    }
  };

  const stopRuntime = async () => {
    setBusy(true);
    setError(null);
    try {
      const snapshot = await clientFacade.stopRuntime();
      applySnapshot(snapshot);
    } catch (cause) {
      setError(displayError(cause));
    } finally {
      setBusy(false);
    }
  };

  const clearDiagnostics = async () => {
    try {
      await clientFacade.clearDiagnostics();
      setDiagnostics([]);
      setClientSnapshot((current) => current && ({
        ...current,
        diagnostics: { unreadCount: 0, lastError: undefined },
      }));
    } catch (cause) {
      setError(displayError(cause));
    }
  };

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
    if (view !== "dashboard") setWorkspaceRulesOpen(false);
    setMobileNavOpen(false);
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
    <div className={`apple-app${mobileNavOpen ? " mobile-nav-open" : ""}`}>
      <header className="global-nav" aria-label="Codex Route navigation">
        <div className="global-nav-inner">
          <button
            className="brand-button"
            type="button"
            aria-label="Codex Route home"
            onClick={() => changeView("dashboard")}
          >
            <img className="brand-logo" src="/codex-route-logo.png" alt="" aria-hidden="true" />
          </button>

          <nav id="global-nav-links" className="global-nav-links" aria-label="Workspace view" role="tablist">
            <button
              className={`nav-link${activeView === "dashboard" ? " active" : ""}`}
              type="button"
              role="tab"
              aria-selected={activeView === "dashboard"}
              onClick={() => changeView("dashboard")}
            >
              <LayoutDashboard size={14} aria-hidden="true" />
              Overview
            </button>
            <button
              className={`nav-link${activeView === "providers" ? " active" : ""}`}
              type="button"
              role="tab"
              aria-selected={activeView === "providers"}
              onClick={() => changeView("providers")}
            >
              <Server size={14} aria-hidden="true" />
              Providers
            </button>
            <button
              className={`nav-link${activeView === "settings" ? " active" : ""}`}
              type="button"
              role="tab"
              aria-selected={activeView === "settings"}
              onClick={() => changeView("settings")}
            >
              <Settings size={14} aria-hidden="true" />
              Settings
            </button>
          </nav>

          <div className="global-nav-actions">
            <div className="global-route-control" aria-label="Route status">
              <span className={`route-context${statusModifier}`} role="status" aria-live="polite">
                <Activity size={14} aria-hidden="true" />
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
            <button
              className="button-dark-utility nav-refresh"
              type="button"
              aria-label="Refresh"
              title="Refresh"
              onClick={() => void refreshManually()}
              disabled={busy}
            >
              <RefreshCw className={busy ? "spin" : undefined} size={15} aria-hidden="true" />
              <span>Refresh</span>
            </button>
            <button
              className="mobile-nav-toggle"
              type="button"
              aria-label={mobileNavOpen ? "Close navigation" : "Open navigation"}
              aria-expanded={mobileNavOpen}
              aria-controls="global-nav-links"
              onClick={() => setMobileNavOpen((open) => !open)}
            >
              {mobileNavOpen ? <X size={18} aria-hidden="true" /> : <Menu size={18} aria-hidden="true" />}
            </button>
          </div>
        </div>
      </header>

      <main className="app-content">
        {activeView === "dashboard" && !clientSnapshot && loading && (
          <section className="client-dashboard client-dashboard-loading" aria-busy="true" aria-labelledby="dashboard-loading-heading">
            <h2 id="dashboard-loading-heading">Loading workspace…</h2>
          </section>
        )}
        {clientSnapshot && activeView === "dashboard" && (
          <DashboardPage
            snapshot={clientSnapshot}
            onProviderChange={(workspace, providerId) => void changeWorkspaceProvider(workspace, providerId)}
            onStartRuntime={() => void startRuntime()}
            onStopRuntime={() => void stopRuntime()}
            onOpenDiagnostics={() => setDiagnosticsOpen(true)}
            workspaceRulesOpen={workspaceRulesOpen}
            onToggleWorkspaceRules={() => setWorkspaceRulesOpen((open) => !open)}
          />
        )}
        {clientSnapshot && (
            <WorkspaceRulesPanel
              providers={providers}
              rules={rules}
              busy={busy}
              onSave={saveRule}
              onRemove={(workspace) => runAction(() => desktopApi.removeRouteRule(workspace))}
              onError={setError}
              open={workspaceRulesOpen}
              onOpenChange={setWorkspaceRulesOpen}
            />
        )}
        {clientSnapshot && activeView === "dashboard" && diagnosticsOpen && (
          <DiagnosticsPanel
            records={diagnostics}
            onClose={() => setDiagnosticsOpen(false)}
            onOpenProviders={() => { setDiagnosticsOpen(false); changeView("providers"); }}
            onOpenWorkspaceRules={() => {
              setDiagnosticsOpen(false);
              changeView("dashboard");
              setWorkspaceRulesOpen(true);
            }}
            onOpenRuntime={() => { setDiagnosticsOpen(false); changeView("providers"); }}
            onClear={() => void clearDiagnostics()}
          />
        )}
        {error && (
          <div className="error-banner" role="alert">
            <span>{error}</span>
            <button className="button-secondary-pill" type="button" onClick={() => void refreshManually()} disabled={busy}>
              Retry
            </button>
          </div>
        )}

        {activeView !== "dashboard" && <div className="workspace-content">
          {activeView === "providers" ? (
            <section className="workspace-panel-region utility-section" aria-labelledby="providers-heading">
              <ProviderPanel
                providers={providers}
                busy={busy}
                loading={loading}
                onSelect={(providerId) => void runAction(() => desktopApi.setCurrentProvider(providerId))}
                onScan={desktopApi.scanCcSwitchProviders}
                onImport={importProviders}
                importOpen={importOpen}
                onImportOpenChange={setImportOpen}
              />
            </section>
          ) : (
            <section className="workspace-panel-region utility-section" aria-labelledby="settings-heading">
              <SettingsPanel
                providers={providers}
                defaultProviderId={clientSnapshot?.provider?.id}
                busy={busy}
                onDefaultProviderChange={(providerId) => void runAction(() => desktopApi.setCurrentProvider(providerId))}
              />
            </section>
          )}
        </div>}
      </main>
    </div>
  );
}

export default App;
