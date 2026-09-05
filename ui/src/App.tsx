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
} from "./api";
import { ProviderPanel } from "./components/ProviderPanel";
import { displayError } from "./errors";
import { clientFacade } from "./clientFacade";
import type { ClientSettings, ClientSnapshot, DiagnosticRecord } from "./api";
import { DashboardPage } from "./components/DashboardPage";
import { DiagnosticsPanel } from "./components/DiagnosticsPanel";
import { SettingsPanel, type SettingsDraft } from "./components/SettingsPanel";
import { applyTheme } from "./theme";
import { createTranslator, I18nProvider, resolveLocale, type Translator } from "./i18n";

const DEFAULT_PORT = 16729;
const DEFAULT_SETTINGS: ClientSettings = {
  autoStart: true,
  startupConsentGranted: false,
  port: DEFAULT_PORT,
  launchAtLogin: false,
  closeToTray: true,
  language: "system",
  theme: "system",
};
type ActiveView = "dashboard" | "providers" | "settings";

type AppProps = { initialSettings?: ClientSettings };

function App({ initialSettings = DEFAULT_SETTINGS }: AppProps) {
  const [providers, setProviders] = useState<ProviderSummary[]>([]);
  const [status, setStatus] = useState<LifecycleStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [clientSettings, setClientSettings] = useState<ClientSettings>({
    ...DEFAULT_SETTINGS,
    ...initialSettings,
    launchAtLogin: initialSettings.launchAtLogin ?? DEFAULT_SETTINGS.launchAtLogin,
    closeToTray: initialSettings.closeToTray ?? DEFAULT_SETTINGS.closeToTray,
    language: initialSettings.language ?? DEFAULT_SETTINGS.language,
    theme: initialSettings.theme ?? DEFAULT_SETTINGS.theme,
  });
  const [port, setPort] = useState(String(initialSettings.port));
  const [settingsProviderId, setSettingsProviderId] = useState("");
  const [activeView, setActiveView] = useState<ActiveView>("dashboard");
  const [importOpen, setImportOpen] = useState(false);
  const [mobileNavOpen, setMobileNavOpen] = useState(false);
  const [clientSnapshot, setClientSnapshot] = useState<ClientSnapshot | null>(null);
  const [diagnostics, setDiagnostics] = useState<DiagnosticRecord[]>([]);
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false);
  const locale = resolveLocale(clientSettings.language ?? "system");
  const t: Translator = useMemo(() => createTranslator(locale), [locale]);

  useEffect(() => applyTheme(clientSettings.theme ?? "system"), [clientSettings.theme]);

  const applySnapshot = useCallback((snapshot: ClientSnapshot) => {
    setClientSnapshot(snapshot);
    setProviders(snapshot.providers);
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
  }, []);

  const currentProvider = useMemo(
    () => providers.find((provider) => provider.isCurrent),
    [providers],
  );

  const routeLabel = useMemo(() => {
    if (!status) return "loadingRoute" as const;
    if (status.externalModification) return "protected" as const;
    if (status.active) return "active" as const;
    return "inactive" as const;
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
    void desktopApi.getClientSettings()
      .then((settings) => {
        setClientSettings(settings);
        setPort(String(settings.port));
      })
      .catch((cause) => setError(displayError(cause)));
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

  useEffect(() => {
    const providerId = clientSnapshot?.provider?.id;
    if (!providerId) return;
    setSettingsProviderId((current) => current || providerId);
  }, [clientSnapshot]);

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
    const numericPort = Number(clientSettings.port);
    if (!Number.isInteger(numericPort) || numericPort < 1 || numericPort > 65535) {
      setError(t("portValidation"));
      return;
    }
    await runAction(() => desktopApi.activateRoute(numericPort));
  };

  const saveSettings = async ({ providerId, port: nextPort, launchAtLogin = clientSettings.launchAtLogin ?? false, closeToTray = clientSettings.closeToTray ?? true, language = clientSettings.language ?? "system", theme = clientSettings.theme ?? "system" }: SettingsDraft) => {
    try {
      const savedProviderId = clientSnapshot?.provider?.id || currentProvider?.id || "";
      let savedProvider: ProviderSummary | undefined;
      const savedSettings = await runBusyAction(async () => {
        if (providerId && providerId !== savedProviderId) {
          savedProvider = await desktopApi.setCurrentProvider(providerId);
        }
        return desktopApi.setClientSettings({ ...clientSettings, port: nextPort, launchAtLogin, closeToTray, language, theme });
      });
      setClientSettings(savedSettings);
      setPort(String(savedSettings.port));
      if (savedProvider) {
        const nextProviders = providers.map((provider) => ({
          ...provider,
          isCurrent: provider.id === savedProvider?.id,
        }));
        setProviders(nextProviders);
        setClientSnapshot((snapshot) => snapshot && ({
          ...snapshot,
          provider: savedProvider,
          providers: nextProviders,
        }));
      }
      setSettingsProviderId(providerId || savedProviderId);
    } catch (cause) {
      setError(displayError(cause));
    }
  };

  const selectCurrentProvider = async (providerId: string) => {
    const succeeded = await runAction(() => desktopApi.setCurrentProvider(providerId));
    if (succeeded) setSettingsProviderId(providerId);
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
    <I18nProvider translator={t}>
    <div
      className={`apple-app${mobileNavOpen ? " mobile-nav-open" : ""}`}
      onContextMenu={(event) => event.preventDefault()}
    >
      <header className="global-nav" aria-label={t("navigation")}>
        <div className="global-nav-inner">
          <div className="global-nav-brand">
            <button
              className="brand-button"
              type="button"
              aria-label={t("home")}
              onClick={() => changeView("dashboard")}
            >
              <img className="brand-logo" src="/codex-route-logo.png" alt="" aria-hidden="true" />
            </button>
          </div>

          <nav id="global-nav-links" className="global-nav-links" aria-label={t("workspaceView")} role="tablist">
            <button
              className={`nav-link${activeView === "dashboard" ? " active" : ""}`}
              type="button"
              role="tab"
              aria-selected={activeView === "dashboard"}
              aria-label={t("overview")}
              onClick={() => changeView("dashboard")}
            >
              <LayoutDashboard size={14} aria-hidden="true" />
              <span className="nav-link-label">{t("overview")}</span>
              {(clientSnapshot?.workspaces.length ?? 0) > 0 && (
                <span className="nav-count" aria-hidden="true">{clientSnapshot?.workspaces.length}</span>
              )}
            </button>
            <button
              className={`nav-link${activeView === "providers" ? " active" : ""}`}
              type="button"
              role="tab"
              aria-selected={activeView === "providers"}
              aria-label={t("providers")}
              onClick={() => changeView("providers")}
            >
              <Server size={14} aria-hidden="true" />
              <span className="nav-link-label">{t("providers")}</span>
              {providers.length > 0 && <span className="nav-count" aria-hidden="true">{providers.length}</span>}
            </button>
            <button
              className={`nav-link${activeView === "settings" ? " active" : ""}`}
              type="button"
              role="tab"
              aria-selected={activeView === "settings"}
              aria-label={t("settings")}
              onClick={() => changeView("settings")}
            >
              <Settings size={14} aria-hidden="true" />
              <span className="nav-link-label">{t("settings")}</span>
            </button>
          </nav>

          <div className="global-nav-actions">
            <div className="global-route-control" aria-label={t("routeStatus")}>
              <span className={`route-context${statusModifier}`} role="status" aria-live="polite">
                <Activity size={14} aria-hidden="true" />
                <span>{t(routeLabel)}</span>
              </span>
              <button
                className={`route-toggle${status?.active ? " active" : ""}`}
                type="button"
                role="switch"
                aria-checked={status?.active === true}
                data-route-state={routeState}
                aria-label={status?.active ? t("deactivateRoute") : t("activateRoute")}
                onClick={toggleRoute}
                disabled={
                  busy ||
                  !status ||
                  (!status.active && currentProvider === undefined) ||
                  status.externalModification
                }
              >
                <span className="route-toggle-track" aria-hidden="true"><span /></span>
                <span className="route-toggle-label">{t("route")}</span>
              </button>
            </div>
            <button
              className="button-dark-utility nav-refresh"
              type="button"
              aria-label={t("refresh")}
              title={t("refresh")}
              onClick={() => void refreshManually()}
              disabled={busy}
            >
              <RefreshCw className={busy ? "spin" : undefined} size={15} aria-hidden="true" />
              <span>{t("refresh")}</span>
            </button>
            <button
              className="mobile-nav-toggle"
              type="button"
              aria-label={mobileNavOpen ? t("closeNavigation") : t("openNavigation")}
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
            <h2 id="dashboard-loading-heading">{t("loadingWorkspace")}</h2>
          </section>
        )}
        {clientSnapshot && activeView === "dashboard" && (
          <DashboardPage
            snapshot={clientSnapshot}
            onProviderChange={(workspace, providerId) => void changeWorkspaceProvider(workspace, providerId)}
          />
        )}
        {clientSnapshot && activeView === "dashboard" && diagnosticsOpen && (
          <DiagnosticsPanel
            records={diagnostics}
            onClose={() => setDiagnosticsOpen(false)}
            onOpenProviders={() => { setDiagnosticsOpen(false); changeView("providers"); }}
            onOpenRuntime={() => { setDiagnosticsOpen(false); changeView("providers"); }}
            onClear={() => void clearDiagnostics()}
          />
        )}
        {error && (
          <div className="error-banner" role="alert">
            <span>{error}</span>
            <button className="button-secondary-pill" type="button" onClick={() => void refreshManually()} disabled={busy}>
              {t("retry")}
            </button>
          </div>
        )}

        {activeView !== "dashboard" && <div className="workspace-content">
          {activeView === "providers" ? (
            <section className="workspace-panel-region utility-section">
              <ProviderPanel
                providers={providers}
                busy={busy}
                loading={loading}
                onSelect={(providerId) => void selectCurrentProvider(providerId)}
                onScan={desktopApi.scanCcSwitchProviders}
                onImport={importProviders}
                importOpen={importOpen}
                onImportOpenChange={setImportOpen}
              />
            </section>
          ) : (
            <section className="workspace-panel-region utility-section">
              <SettingsPanel
                providers={providers}
                settings={clientSettings}
                t={t}
                defaultProviderId={settingsProviderId || clientSnapshot?.provider?.id}
                port={port}
                busy={busy}
                onDefaultProviderChange={setSettingsProviderId}
                onPortChange={setPort}
                onSave={saveSettings}
              />
            </section>
          )}
        </div>}
      </main>
    </div>
    </I18nProvider>
  );
}

export default App;
