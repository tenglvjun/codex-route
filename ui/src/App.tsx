import { useCallback, useEffect, useState } from "react";
import { desktopApi, type LifecycleStatus, type ProviderSummary } from "./api";

function App() {
  const [providers, setProviders] = useState<ProviderSummary[]>([]);
  const [status, setStatus] = useState<LifecycleStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const [nextProviders, nextStatus] = await Promise.all([
        desktopApi.listProviders(),
        desktopApi.getLifecycleStatus(),
      ]);
      setProviders(nextProviders);
      setStatus(nextStatus);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const runAction = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="shell">
      <header className="header">
        <div>
          <p className="eyebrow">LOCAL DESKTOP CLIENT</p>
          <h1>Codex Route</h1>
          <p className="subtitle">Manage the local Codex route without exposing provider credentials.</p>
        </div>
        <button className="button secondary" onClick={() => void refresh()} disabled={busy}>
          Refresh
        </button>
      </header>

      {error && <div className="error" role="alert">{error}</div>}

      <section className="status-card" aria-labelledby="status-heading">
        <div>
          <p className="eyebrow" id="status-heading">ROUTE STATUS</p>
          <strong className={status?.active ? "status active" : "status"}>
            {status?.status ?? "loading"}
          </strong>
          <p className="muted">
            {status?.port ? `127.0.0.1:${status.port}` : "No listener"}
            {status?.configManaged ? " · Codex config managed" : ""}
          </p>
        </div>
        <div className="actions">
          <button
            className="button primary"
            onClick={() => void runAction(() => desktopApi.activateRoute())}
            disabled={busy || status?.active === true}
          >
            Activate
          </button>
          <button
            className="button danger"
            onClick={() => void runAction(() => desktopApi.deactivateRoute())}
            disabled={busy || status?.active !== true}
          >
            Deactivate
          </button>
        </div>
      </section>

      <section className="panel" aria-labelledby="providers-heading">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">PROVIDERS</p>
            <h2 id="providers-heading">Current provider</h2>
          </div>
          <span className="count">{providers.length}</span>
        </div>
        {providers.length === 0 ? (
          <p className="muted">No providers found. Import one with the CLI before using the desktop client.</p>
        ) : (
          <div className="provider-list">
            {providers.map((provider) => (
              <div className="provider-row" key={provider.id}>
                <div>
                  <strong>{provider.name}</strong>
                  <span className="muted">{provider.id} · {provider.source}</span>
                </div>
                <button
                  className={provider.isCurrent ? "badge current" : "button secondary"}
                  onClick={() => void runAction(() => desktopApi.setCurrentProvider(provider.id))}
                  disabled={busy || provider.isCurrent}
                >
                  {provider.isCurrent ? "Current" : "Use provider"}
                </button>
              </div>
            ))}
          </div>
        )}
      </section>
    </main>
  );
}

export default App;
