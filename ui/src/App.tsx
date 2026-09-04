import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Activity,
  FileInput,
  FolderTree,
  Menu,
  Plus,
  RefreshCw,
  Server,
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
import { RouteStatusPanel } from "./components/RouteStatusPanel";
import { WorkspaceRulesPanel } from "./components/WorkspaceRulesPanel";
import { displayError } from "./errors";

const DEFAULT_PORT = 16729;
type ActiveView = "providers" | "rules";

function App() {
  const [providers, setProviders] = useState<ProviderSummary[]>([]);
  const [rules, setRules] = useState<WorkspaceRouteRule[]>([]);
  const [status, setStatus] = useState<LifecycleStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [port, setPort] = useState(String(DEFAULT_PORT));
  const [activeView, setActiveView] = useState<ActiveView>("providers");
  const [importOpen, setImportOpen] = useState(false);
  const [mobileNavOpen, setMobileNavOpen] = useState(false);
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
    setLoading(true);
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
    } finally {
      if (version === refreshVersion.current) setLoading(false);
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
            onClick={() => changeView("providers")}
          >
            <img className="brand-logo" src="/codex-route-logo.png" alt="" aria-hidden="true" />
          </button>

          <nav id="global-nav-links" className="global-nav-links" aria-label="Workspace view" role="tablist">
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
              className={`nav-link${activeView === "rules" ? " active" : ""}`}
              type="button"
              role="tab"
              aria-selected={activeView === "rules"}
              onClick={() => changeView("rules")}
            >
              <FolderTree size={14} aria-hidden="true" />
              Workspace rules
            </button>
          </nav>

          <div className="global-nav-actions">
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

      <div className="sub-nav-frosted" aria-label="Codex Route workspace bar">
        <div className="sub-nav-inner">
          <img className="brand-logo" src="/codex-route-logo.png" alt="" aria-hidden="true" />
          <div className="sub-nav-title-group">
            <strong>Codex Route</strong>
            <span>{activeView === "providers" ? "Providers" : "Workspace rules"}</span>
          </div>
          <div className="sub-nav-route">
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
            className="button-primary sub-nav-cta"
            type="button"
            aria-label={activeView === "providers" ? "Import providers" : "Add workspace rule"}
            onClick={() => activeView === "providers" ? setImportOpen(true) : changeView("rules")}
            disabled={busy}
          >
            {activeView === "providers" ? <FileInput size={15} aria-hidden="true" /> : <Plus size={15} aria-hidden="true" />}
            <span>{activeView === "providers" ? "Import" : "Add rule"}</span>
          </button>
        </div>
      </div>

      <main className="app-content">
        <section className={`workspace-heading product-tile ${activeView === "providers" ? "product-tile-light" : "product-tile-parchment"}`}>
          <div className="workspace-heading-inner">
          <div>
            <p className="eyebrow">LOCAL WORKSPACE</p>
            <h1>{activeView === "providers" ? "Providers" : "Workspace rules"}</h1>
            <p className="lead">
              {activeView === "providers"
                ? "Choose the provider Codex should use for local requests."
                : "Route each Codex workspace to its preferred provider."}
            </p>
            <div className="hero-actions">
              <button
                className="button-primary"
                type="button"
                aria-label={activeView === "providers" ? "Open provider import" : undefined}
                onClick={() => activeView === "providers" ? setImportOpen(true) : changeView("rules")}
                disabled={busy}
              >
                {activeView === "providers" ? <FileInput size={16} aria-hidden="true" /> : <Plus size={16} aria-hidden="true" />}
                {activeView === "providers" ? "Import providers" : "Add workspace rule"}
              </button>
              <button
                className="button-secondary-pill"
                type="button"
                onClick={() => void refreshManually()}
                disabled={busy}
              >
                <RefreshCw className={busy ? "spin" : undefined} size={15} aria-hidden="true" />
                Refresh
              </button>
            </div>
          </div>
          <div className="workspace-signal" aria-label="Workspace summary">
            <span className="workspace-signal-value">{providers.length}</span>
            <span className="workspace-signal-label">providers</span>
            <span className="workspace-signal-divider" aria-hidden="true" />
            <span className="workspace-signal-value">{rules.length}</span>
            <span className="workspace-signal-label">rules</span>
            {currentProvider && <span className="current-provider">Using {currentProvider.name}</span>}
          </div>
        </div>
        </section>

        {error && (
          <div className="error-banner" role="alert">
            <span>{error}</span>
            <button className="button-secondary-pill" type="button" onClick={() => void refreshManually()} disabled={busy}>
              Retry
            </button>
          </div>
        )}

        <div className="workspace-content">
          {activeView === "providers" ? (
            <>
              <section className="route-panel-region product-tile product-tile-dark" aria-label="Route configuration">
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
              <section className="workspace-panel-region utility-section" aria-labelledby="providers-heading">
                <ProviderPanel
                  providers={providers}
                  busy={busy}
                  loading={loading}
                  onSelect={(providerId) => void runAction(() => desktopApi.setCurrentProvider(providerId))}
                  onImport={importProviders}
                  onError={setError}
                  importOpen={importOpen}
                  onImportOpenChange={setImportOpen}
                />
              </section>
            </>
          ) : (
            <section className="workspace-panel-region utility-section" aria-labelledby="rules-heading">
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
