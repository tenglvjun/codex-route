import { invoke } from "@tauri-apps/api/core";

export type RuntimePhase =
  | "stopped"
  | "starting"
  | "running"
  | "degraded"
  | "recovering"
  | "blocked_external_modification"
  | "failed";

export type RuntimeSnapshot = {
  phase: RuntimePhase;
  active: boolean;
  pid?: number;
  port?: number;
  serverReachable: boolean;
  configManaged: boolean;
  externalModification: boolean;
  lastError?: string;
  restartCount: number;
  updatedAt: number;
  sequence: number;
};

export type CodexStatus = {
  home: string;
  configPath: string;
  installed: boolean;
  version?: string;
  configExists: boolean;
  configManaged: boolean;
  externalModification: boolean;
};

export type WorkspaceSnapshot = {
  path: string;
  exists: boolean;
  sessionId: string;
  sessionIds: string[];
  threadIds: string[];
  providerId?: string;
  lastActivity?: number;
  conflictingWorkspaces: boolean;
};

export type DiagnosticsSummary = {
  unreadCount: number;
  lastError?: string;
};

export type ClientSnapshot = {
  schemaVersion: number;
  sequence: number;
  generatedAt: number;
  codex: CodexStatus;
  workspaces: WorkspaceSnapshot[];
  workspace?: WorkspaceSnapshot;
  provider?: ProviderSummary;
  providers: ProviderSummary[];
  rules: WorkspaceRouteRule[];
  runtime: RuntimeSnapshot;
  diagnostics: DiagnosticsSummary;
};

export type ClientSettings = {
  autoStart: boolean;
  startupConsentGranted: boolean;
  port: number;
  launchAtLogin: boolean;
  closeToTray: boolean;
  language: LanguagePreference;
  theme: ThemePreference;
};

export type LanguagePreference = "system" | "zh-CN" | "zh-TW" | "en";
export type ThemePreference = "system" | "light" | "dark";

export type DiagnosticRecord = {
  id: number;
  timestamp: number;
  severity: "info" | "warning" | "error";
  code: string;
  message: string;
  source: string;
  context: Record<string, string>;
};

export type ProviderSummary = {
  id: string;
  name: string;
  category?: string;
  source: string;
  isCurrent: boolean;
};

export type LifecycleStatus = {
  status: string;
  active: boolean;
  pid?: number;
  port?: number;
  serverReachable: boolean;
  configManaged: boolean;
  externalModification: boolean;
  configPath: string;
  statePath: string;
  lockPath: string;
};

export type WorkspaceRouteRule = {
  workspace: string;
  providerId: string;
  createdAt: number;
  updatedAt: number;
};

export type UpsertRouteRuleRequest = {
  workspace: string;
  providerId: string;
  replace?: boolean;
};

export type ActivationResult = {
  status: string;
  pid: number;
  port: number;
  routeUrl: string;
  configPath: string;
  statePath: string;
  lockPath: string;
};

export type ConflictPolicy = "skip" | "replace" | "rename";

export type ImportCcSwitchRequest = {
  providerIds: string[];
  conflictPolicy: ConflictPolicy;
};

export type RejectedProvider = {
  id: string;
  reason: string;
};

export type ImportReport = {
  source: string;
  imported: number;
  replaced: number;
  renamed: number;
  skipped: number;
  rejected: RejectedProvider[];
};

export type CcSwitchProviderCandidate = {
  id: string;
  name: string;
  category?: string;
  alreadyImported: boolean;
};

export type CcSwitchScanReport = {
  source: string;
  providers: CcSwitchProviderCandidate[];
  rejected: RejectedProvider[];
};

export const desktopApi = {
  getClientSnapshot: () => invoke<ClientSnapshot>("get_client_snapshot"),
  getClientSettings: () => invoke<ClientSettings>("get_client_settings"),
  setClientSettings: (settings: ClientSettings) =>
    invoke<ClientSettings>("set_client_settings", { settings }),
  startRuntime: () => invoke<ClientSnapshot>("start_runtime"),
  stopRuntime: () => invoke<ClientSnapshot>("stop_runtime"),
  setWorkspaceProvider: (workspace: string, providerId: string) =>
    invoke<ClientSnapshot>("set_workspace_provider", {
      request: { workspace, providerId },
    }),
  getDiagnostics: (limit?: number) =>
    invoke<DiagnosticRecord[]>("get_diagnostics", { limit }),
  clearDiagnostics: () => invoke<void>("clear_diagnostics"),
  listProviders: () => invoke<ProviderSummary[]>("list_providers"),
  setCurrentProvider: (providerId: string) =>
    invoke<ProviderSummary>("set_current_provider", { providerId }),
  scanCcSwitchProviders: () => invoke<CcSwitchScanReport>("scan_cc_switch_providers"),
  importCcSwitchProviders: (request: ImportCcSwitchRequest) =>
    invoke<ImportReport>("import_cc_switch_providers", { request }),
  getLifecycleStatus: () => invoke<LifecycleStatus>("get_lifecycle_status"),
  listRouteRules: () => invoke<WorkspaceRouteRule[]>("list_route_rules"),
  upsertRouteRule: (request: UpsertRouteRuleRequest) =>
    invoke<WorkspaceRouteRule>("upsert_route_rule", { request }),
  removeRouteRule: (workspace: string) =>
    invoke<WorkspaceRouteRule>("remove_route_rule", { workspace }),
  activateRoute: (port?: number) =>
    invoke<ActivationResult>("activate_route", { request: { port } }),
  deactivateRoute: () => invoke("deactivate_route"),
};
