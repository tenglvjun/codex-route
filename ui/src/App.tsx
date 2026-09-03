import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { FolderTree, LayoutDashboard, Menu, RefreshCw, Server, X } from "lucide-react";
import { desktopApi, type ImportCcSwitchRequest, type ImportReport, type LifecycleStatus, type ProviderSummary, type UpsertRouteRuleRequest, type WorkspaceRouteRule } from "./api";
import { ProviderPanel } from "./components/ProviderPanel";
import { RouteStatusPanel } from "./components/RouteStatusPanel";
import { WorkspaceRulesPanel } from "./components/WorkspaceRulesPanel";
import { displayError } from "./errors";

const DEFAULT_PORT = 16729;

function App() {
  const [providers, setProviders] = useState<ProviderSummary[]>([]);
  const [rules, setRules] = useState<WorkspaceRouteRule[]>([]);
  const [status, setStatus] = useState<LifecycleStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [port, setPort] = useState(String(DEFAULT_PORT));
  const [mobileNavOpen, setMobileNavOpen] = useState(false);
  const refreshVersion = useRef(0);

  const currentProvider = useMemo(
    () => providers.find((provider) => provider.isCurrent),
    [providers],
  );

  const routeLabel = useMemo(() => {
    if (!status) return "Route loading";
    if (status.externalModification) return "Route protected";
    if (status.active) return "Route active";
    return `Route ${status.status}`;
  }, [status]);
  const providerLabel = currentProvider ? `Fallback: ${currentProvider.name}` : "Fallback: None selected";

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

  const closeMobileNav = () => setMobileNavOpen(false);

  return (
    <div className={`app-shell${mobileNavOpen ? " mobile-nav-open" : ""}`}>
      <aside className="app-rail" aria-label="Primary navigation">
        <div className="rail-brand">
          <span className="brand-mark" aria-hidden="true">CR</span>
          <span className="brand-name">Codex Route</span>
        </div>
        <nav className="rail-nav" id="primary-navigation" aria-label="Main navigation">
          <a href="#overview" onClick={closeMobileNav}>
            <LayoutDashboard size={18} aria-hidden="true" />
            <span>Overview</span>
          </a>
          <a href="#providers" onClick={closeMobileNav}>
            <Server size={18} aria-hidden="true" />
            <span>Providers</span>
          </a>
          <a href="#workspace-rules" onClick={closeMobileNav}>
            <FolderTree size={18} aria-hidden="true" />
            <span>Workspace rules</span>
          </a>
        </nav>
        <div className="rail-footer" aria-label="Route context">
          <span className="rail-footer-label">Local desktop client</span>
          <strong>{routeLabel}</strong>
          <span>{providerLabel}</span>
        </div>
      </aside>

      <div className="app-main">
        <header className="mobile-topbar">
          <div className="rail-brand">
            <span className="brand-mark" aria-hidden="true">CR</span>
            <span className="brand-name">Codex Route</span>
          </div>
          <button
            className="icon-button mobile-menu-button"
            type="button"
            aria-label={mobileNavOpen ? "Close navigation" : "Open navigation"}
            aria-expanded={mobileNavOpen}
            aria-controls="primary-navigation"
            onClick={() => setMobileNavOpen((open) => !open)}
          >
            {mobileNavOpen ? <X size={20} aria-hidden="true" /> : <Menu size={20} aria-hidden="true" />}
          </button>
        </header>

        <main className="shell content">
          <header className="header content-header">
            <div className="header-copy">
              <p className="eyebrow">LOCAL DESKTOP CLIENT</p>
              <h1>Codex Route</h1>
              <p className="subtitle">Local Codex provider routing</p>
              <div className="header-context" aria-label="Current route context">
                <span>{routeLabel}</span>
                <span>{providerLabel}</span>
              </div>
            </div>
            <button className="button secondary" onClick={() => void refreshManually()} disabled={busy}>
              <RefreshCw className={busy ? "spin" : undefined} size={16} aria-hidden="true" />
              {busy ? "Working..." : "Refresh"}
            </button>
          </header>

          {error && <div className="error" role="alert">{error}</div>}

          <section id="overview" className="overview-section" aria-labelledby="status-heading">
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
          <div className="content-grid">
            <section id="providers" className="content-section" aria-labelledby="providers-heading">
              <ProviderPanel
                providers={providers}
                busy={busy}
                onSelect={(providerId) => void runAction(() => desktopApi.setCurrentProvider(providerId))}
                onImport={importProviders}
                onError={setError}
              />
            </section>
            <section id="workspace-rules" className="content-section" aria-labelledby="rules-heading">
              <WorkspaceRulesPanel
                providers={providers}
                rules={rules}
                busy={busy}
                onSave={saveRule}
                onRemove={(workspace) => runAction(() => desktopApi.removeRouteRule(workspace))}
                onError={setError}
              />
            </section>
          </div>
        </main>
      </div>
    </div>
  );
}

export default App;
