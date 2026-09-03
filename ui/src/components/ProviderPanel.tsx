import { Power } from "lucide-react";
import type { ProviderSummary } from "../api";

type ProviderPanelProps = {
  providers: ProviderSummary[];
  busy: boolean;
  onSelect: (providerId: string) => void;
};

export function ProviderPanel({ providers, busy, onSelect }: ProviderPanelProps) {
  return (
    <section className="panel" aria-labelledby="providers-heading">
      <div className="panel-heading">
        <div>
          <p className="eyebrow">PROVIDERS</p>
          <h2 id="providers-heading">Default fallback provider</h2>
        </div>
        <span className="count">{providers.length}</span>
      </div>
      {providers.length === 0 ? (
        <p className="muted">No providers available.</p>
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
                onClick={() => onSelect(provider.id)}
                disabled={busy || provider.isCurrent}
              >
                {!provider.isCurrent && <Power size={15} aria-hidden="true" />}
                {provider.isCurrent ? "Current" : "Use provider"}
              </button>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
