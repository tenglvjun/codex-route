import { Server } from "lucide-react";
import type { ProviderSummary } from "../api";
import { useTranslation } from "../i18n";
import { ProviderSelect } from "./ProviderSelect";

type QuickProviderSwitchProps = {
  providers: ProviderSummary[];
  providerId: string;
  onChange?: (providerId: string) => void;
};

export function QuickProviderSwitch({ providers, providerId, onChange }: QuickProviderSwitchProps) {
  const t = useTranslation();
  return (
    <div className="dashboard-card dashboard-card-provider">
      <div className="dashboard-card-icon" aria-hidden="true"><Server size={18} /></div>
      <div>
        <span>{t("provider")}</span>
        <ProviderSelect
          providers={providers}
          selectedProviderId={providerId}
          onChange={(nextProviderId) => onChange?.(nextProviderId)}
          ariaLabel={t("provider")}
          emptyOptionLabel={t("chooseProvider")}
          allowEmptyOption
          disabled={providers.length === 0 || !onChange}
        />
      </div>
    </div>
  );
}
