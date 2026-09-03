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

export const desktopApi = {
  listProviders: () => invoke<ProviderSummary[]>("list_providers"),
  setCurrentProvider: (providerId: string) =>
    invoke<ProviderSummary>("set_current_provider", { providerId }),
  getLifecycleStatus: () => invoke<LifecycleStatus>("get_lifecycle_status"),
  activateRoute: (port?: number) =>
    invoke("activate_route", { request: { port } }),
  deactivateRoute: () => invoke("deactivate_route"),
};
