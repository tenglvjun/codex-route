import { Server } from "lucide-react";
import type { ProviderSummary } from "../api";

type QuickProviderSwitchProps = {
  providers: ProviderSummary[];
  providerId: string;
  onChange?: (providerId: string) => void;
};

export function QuickProviderSwitch({ providers, providerId, onChange }: QuickProviderSwitchProps) {
  return (
    <div className="dashboard-card dashboard-card-provider">
      <div className="dashboard-card-icon" aria-hidden="true"><Server size={18} /></div>
      <div>
        <label htmlFor="dashboard-provider">Provider</label>
        <select id="dashboard-provider" value={providerId} onChange={(event) => onChange?.(event.target.value)} disabled={providers.length === 0 || !onChange}>
          <option value="">Choose provider</option>
          {providers.map((provider) => <option value={provider.id} key={provider.id}>{provider.name}</option>)}
        </select>
      </div>
    </div>
  );
}
