import { invoke } from "@tauri-apps/api/core";

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
