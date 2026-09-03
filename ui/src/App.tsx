import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { RefreshCw } from "lucide-react";
import { desktopApi, type LifecycleStatus, type ProviderSummary, type UpsertRouteRuleRequest, type WorkspaceRouteRule } from "./api";
import { ProviderPanel } from "./components/ProviderPanel";
import { RouteStatusPanel } from "./components/RouteStatusPanel";
import { WorkspaceRulesPanel } from "./components/WorkspaceRulesPanel";

const DEFAULT_PORT = 16729;

function displayError(cause: unknown) {
  return cause instanceof Error ? cause.message : String(cause);
}

function App() {
  const [providers, setProviders] = useState<ProviderSummary[]>([]);
  const [rules, setRules] = useState<WorkspaceRouteRule[]>([]);
  const [status, setStatus] = useState<LifecycleStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [port, setPort] = useState(String(DEFAULT_PORT));
  const refreshVersion = useRef(0);

  const currentProvider = useMemo(
    () => providers.find((provider) => provider.isCurrent),
    [providers],
  );

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

  const runAction = async (action: () => Promise<unknown>): Promise<boolean> => {
    setBusy(true);
    setError(null);
    try {
      await action();
      await refresh();
      return true;
    } catch (cause) {
      setError(displayError(cause));
      return false;
    } finally {
      setBusy(false);
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

  const saveRule = async (request: UpsertRouteRuleRequest) => {
    setBusy(true);
    setError(null);
    try {
      await desktopApi.upsertRouteRule(request);
      await refresh();
    } finally {
      setBusy(false);
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

  return (
    <main className="shell">
      <header className="header">
        <div>
          <p className="eyebrow">LOCAL DESKTOP CLIENT</p>
          <h1>Codex Route</h1>
          <p className="subtitle">Local Codex provider routing</p>
        </div>
        <button className="button secondary" onClick={() => void refreshManually()} disabled={busy}>
          <RefreshCw className={busy ? "spin" : undefined} size={16} aria-hidden="true" />
          {busy ? "Working..." : "Refresh"}
        </button>
      </header>

      {error && <div className="error" role="alert">{error}</div>}

      <RouteStatusPanel
        status={status}
        port={port}
        busy={busy}
        canActivate={currentProvider !== undefined}
        onPortChange={setPort}
        onActivate={() => void activate()}
        onDeactivate={() => void runAction(() => desktopApi.deactivateRoute())}
      />
      <ProviderPanel
        providers={providers}
        busy={busy}
        onSelect={(providerId) => void runAction(() => desktopApi.setCurrentProvider(providerId))}
      />
      <WorkspaceRulesPanel
        providers={providers}
        rules={rules}
        busy={busy}
        onSave={saveRule}
        onRemove={(workspace) => runAction(() => desktopApi.removeRouteRule(workspace))}
        onError={setError}
      />
    </main>
  );
}

export default App;
