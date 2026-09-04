import { Route, Settings as SettingsIcon } from "lucide-react";
import type { ProviderSummary } from "../api";

type SettingsPanelProps = {
  providers: ProviderSummary[];
  defaultProviderId?: string;
  busy: boolean;
  onDefaultProviderChange: (providerId: string) => void;
};

export function SettingsPanel({
  providers,
  defaultProviderId = "",
  busy,
  onDefaultProviderChange,
}: SettingsPanelProps) {
  return (
    <section className="panel settings-panel" aria-labelledby="settings-heading">
      <div className="panel-heading">
        <div className="panel-heading-title">
          <SettingsIcon size={18} aria-hidden="true" />
          <h2 id="settings-heading">Settings</h2>
        </div>
      </div>
      <div className="settings-list">
        <div className="settings-row">
          <div className="settings-copy">
            <strong>Default route</strong>
            <span>Used when a session has no known workspace rule.</span>
          </div>
          <label className="settings-select">
            <span className="sr-only">Default route provider</span>
            <select
              aria-label="Default route provider"
              value={defaultProviderId}
              onChange={(event) => onDefaultProviderChange(event.target.value)}
              disabled={busy || providers.length === 0}
            >
              <option value="">Choose provider</option>
              {providers.map((provider) => (
                <option value={provider.id} key={provider.id}>{provider.name}</option>
              ))}
            </select>
          </label>
        </div>
        <div className="settings-note">
          <Route size={16} aria-hidden="true" />
          <span>Workspace-specific routes override this default.</span>
        </div>
      </div>
    </section>
  );
}
