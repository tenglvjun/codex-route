import { useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";
import { Check, Globe2, MonitorDown, Network, Palette, Rocket, Route } from "lucide-react";
import type { ClientSettings, LanguagePreference, ProviderSummary, ThemePreference } from "../api";
import { ProviderSelect } from "./ProviderSelect";
import { PreferenceSelect } from "./PreferenceSelect";
import { createTranslator, type Translator } from "../i18n";

export type SettingsDraft = {
  providerId: string;
  port: number;
  launchAtLogin: boolean;
  closeToTray: boolean;
  language: LanguagePreference;
  theme: ThemePreference;
};

type SettingsPanelProps = {
  providers: ProviderSummary[];
  settings: ClientSettings;
  defaultProviderId?: string;
  port: string;
  busy: boolean;
  onDefaultProviderChange: (providerId: string) => void;
  onPortChange: (port: string) => void;
  onSave: (settings: SettingsDraft) => void | Promise<void>;
  t?: Translator;
};

export function SettingsPanel({
  providers,
  settings,
  defaultProviderId = "",
  port,
  busy,
  onDefaultProviderChange,
  onPortChange,
  onSave,
  t,
}: SettingsPanelProps) {
  const copy = t ?? createTranslator("en");
  const [portError, setPortError] = useState<string | null>(null);
  const providerIdRef = useRef(defaultProviderId);
  const [launchAtLogin, setLaunchAtLogin] = useState(settings.launchAtLogin);
  const [closeToTray, setCloseToTray] = useState(settings.closeToTray);
  const [language, setLanguage] = useState<LanguagePreference>(settings.language);
  const [theme, setTheme] = useState<ThemePreference>(settings.theme);
  const [draftPort, setDraftPort] = useState(port);

  useEffect(() => {
    providerIdRef.current = defaultProviderId;
    setLaunchAtLogin(settings.launchAtLogin);
    setCloseToTray(settings.closeToTray);
    setLanguage(settings.language);
    setTheme(settings.theme);
  }, [defaultProviderId, settings]);

  useEffect(() => {
    setDraftPort(port);
  }, [port]);

  const changeDefaultProvider = (providerId: string) => {
    providerIdRef.current = providerId;
    onDefaultProviderChange(providerId);
  };

  const saveSettings = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const numericPort = Number(draftPort);
    if (!Number.isInteger(numericPort) || numericPort < 1 || numericPort > 65535) {
      setPortError(copy("portValidation"));
      return;
    }
    setPortError(null);
    const draft: SettingsDraft = { providerId: providerIdRef.current, port: numericPort, launchAtLogin, closeToTray, language, theme };
    void onSave(draft);
  };

  return (
    <section className="panel settings-panel" aria-label={copy("settings")}>
      <form className="settings-form" onSubmit={saveSettings} noValidate>
        <header className="settings-header">
          <p className="eyebrow">{copy("settings")}</p>
          <h1>{copy("settings")}</h1>
          <p>{copy("settingsDescription")}</p>
        </header>

        <div className="settings-list">
          <SettingsSection
            id="settings-startup"
            title={copy("startupBehavior")}
            description={copy("startupBehaviorDescription")}
          >
            <PreferenceRow icon={<Rocket size={18} aria-hidden="true" />} label={copy("launchAtLogin")} description={copy("launchAtLoginDescription")}>
              <button className="settings-toggle" type="button" role="switch" aria-checked={launchAtLogin} data-state={launchAtLogin ? "on" : "off"} aria-label={copy("launchAtLogin")} onClick={() => setLaunchAtLogin((value) => !value)} disabled={busy}><span className="settings-toggle-track"><span className="settings-toggle-thumb" /></span></button>
            </PreferenceRow>
            <PreferenceRow icon={<MonitorDown size={18} aria-hidden="true" />} label={copy("closeToTray")} description={copy("closeToTrayDescription")}>
              <button className="settings-toggle" type="button" role="switch" aria-checked={closeToTray} data-state={closeToTray ? "on" : "off"} aria-label={copy("closeToTray")} onClick={() => setCloseToTray((value) => !value)} disabled={busy}><span className="settings-toggle-track"><span className="settings-toggle-thumb" /></span></button>
            </PreferenceRow>
          </SettingsSection>

          <SettingsSection
            id="settings-interface"
            title={copy("interfacePreferences")}
            description={copy("interfacePreferencesDescription")}
            className="settings-section-grid"
          >
            <PreferenceCard icon={<Globe2 size={18} aria-hidden="true" />} label={copy("language")} description={copy("languageDescription")}>
              <PreferenceSelect value={language} onChange={setLanguage} ariaLabel={copy("language")} disabled={busy} options={[{ value: "system", label: copy("system") }, { value: "zh-CN", label: copy("simplifiedChinese") }, { value: "zh-TW", label: copy("traditionalChinese") }, { value: "en", label: copy("english") }]} />
            </PreferenceCard>
            <PreferenceCard icon={<Palette size={18} aria-hidden="true" />} label={copy("theme")} description={copy("themeDescription")}>
              <PreferenceSelect value={theme} onChange={setTheme} ariaLabel={copy("theme")} disabled={busy} options={[{ value: "system", label: copy("system") }, { value: "light", label: copy("light") }, { value: "dark", label: copy("dark") }]} />
            </PreferenceCard>
          </SettingsSection>

          <SettingsSection
            id="settings-route"
            title={copy("routeSettings")}
            description={copy("routeSettingsDescription")}
            className="settings-section-grid"
          >
            <PreferenceCard icon={<Route size={18} aria-hidden="true" />} label={copy("defaultRoute")} description={copy("defaultRouteDescription")}>
              <ProviderSelect
                providers={providers}
                selectedProviderId={defaultProviderId}
                onChange={changeDefaultProvider}
                ariaLabel={copy("defaultRouteProvider")}
                emptyOptionLabel={copy("chooseProvider")}
                className="settings-provider-select"
                disabled={busy}
              />
            </PreferenceCard>
            <PreferenceCard icon={<Network size={18} aria-hidden="true" />} label={copy("routePort")} description={copy("routePortDescription")}>
              <div className="settings-control settings-port-control">
                <label className="settings-input">
                  <span className="sr-only">{copy("routePort")}</span>
                  <input
                    aria-label={copy("routePort")}
                    type="number"
                    min="1"
                    max="65535"
                    inputMode="numeric"
                    value={draftPort}
                    onChange={(event) => {
                      setDraftPort(event.target.value);
                      onPortChange(event.target.value);
                      if (portError) setPortError(null);
                    }}
                    disabled={busy}
                  />
                </label>
                {portError && <span className="field-error settings-port-error" role="alert">{portError}</span>}
              </div>
            </PreferenceCard>
          </SettingsSection>

          <div className="settings-note">
            <Route size={16} aria-hidden="true" />
            <span>{copy("workspaceNote")}</span>
          </div>
        </div>
        <div className="settings-actions">
          <span className="settings-actions-copy">{copy("changesAppliedTogether")}</span>
          <button className="button-primary settings-save-button" type="submit" disabled={busy}>
            <Check size={15} aria-hidden="true" />
            {copy("saveChanges")}
          </button>
        </div>
      </form>
    </section>
  );
}

function SettingsSection({ id, title, description, className = "", children }: { id: string; title: string; description: string; className?: string; children: ReactNode }) {
  return <section className={`settings-section ${className}`} aria-labelledby={`${id}-heading`}><header className="settings-section-heading"><div><h2 id={`${id}-heading`}>{title}</h2><p>{description}</p></div></header><div className="settings-section-body">{children}</div></section>;
}

function PreferenceRow({ icon, label, description, children }: { icon: ReactNode; label: string; description: string; children: ReactNode }) {
  return <div className="settings-row settings-preference-row"><span className="settings-item-icon" aria-hidden="true">{icon}</span><div className="settings-copy"><strong>{label}</strong><span>{description}</span></div><div className="settings-control">{children}</div></div>;
}

function PreferenceCard({ icon, label, description, children }: { icon: ReactNode; label: string; description: string; children: ReactNode }) {
  return <div className="settings-card"><div className="settings-card-heading"><span className="settings-item-icon" aria-hidden="true">{icon}</span><div className="settings-copy"><strong>{label}</strong><span>{description}</span></div></div><div className="settings-card-control">{children}</div></div>;
}
