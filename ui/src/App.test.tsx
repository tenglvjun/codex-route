// @vitest-environment jsdom

import { cleanup, createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { desktopApi, type ClientSnapshot, type LifecycleStatus, type ProviderSummary } from "./api";
import { clientFacade } from "./clientFacade";
import App from "./App";

vi.mock("./api", () => ({
  desktopApi: {
    listProviders: vi.fn(),
    listRouteRules: vi.fn(),
    getLifecycleStatus: vi.fn(),
    setCurrentProvider: vi.fn(),
    scanCcSwitchProviders: vi.fn(),
    importCcSwitchProviders: vi.fn(),
    upsertRouteRule: vi.fn(),
    removeRouteRule: vi.fn(),
    activateRoute: vi.fn(),
    deactivateRoute: vi.fn(),
    getClientSnapshot: vi.fn(),
    getDiagnostics: vi.fn(),
    startRuntime: vi.fn(),
    stopRuntime: vi.fn(),
    setWorkspaceProvider: vi.fn(),
    clearDiagnostics: vi.fn(),
    getClientSettings: vi.fn(),
    setClientSettings: vi.fn(),
  },
}));

vi.mock("./clientFacade", () => ({
  clientFacade: {
    loadSnapshot: vi.fn(),
    getDiagnostics: vi.fn(),
    subscribe: vi.fn(),
    subscribeDiagnostics: vi.fn(),
    startRuntime: vi.fn(),
    stopRuntime: vi.fn(),
    setWorkspaceProvider: vi.fn(),
    clearDiagnostics: vi.fn(),
  },
}));

const provider: ProviderSummary = {
  id: "provider-a",
  name: "Provider A",
  source: "local",
  isCurrent: true,
};

const inactiveStatus: LifecycleStatus = {
  status: "inactive",
  active: false,
  serverReachable: false,
  configManaged: false,
  externalModification: false,
  configPath: "/tmp/config.toml",
  statePath: "/tmp/route-state.json",
  lockPath: "/tmp/route.lock",
};

const clientSnapshot: ClientSnapshot = {
  schemaVersion: 1,
  sequence: 1,
  generatedAt: 1,
  codex: {
    home: "/tmp/.codex",
    configPath: "/tmp/.codex/config.toml",
    installed: true,
    configExists: true,
    configManaged: false,
    externalModification: false,
  },
  workspaces: [],
  workspace: undefined,
  provider,
  providers: [provider],
  rules: [],
  runtime: {
    phase: "stopped",
    active: false,
    serverReachable: false,
    configManaged: false,
    externalModification: false,
    restartCount: 0,
    updatedAt: 1,
    sequence: 1,
  },
  diagnostics: { unreadCount: 0 },
};

afterEach(cleanup);

describe("App", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(desktopApi.listProviders).mockResolvedValue([provider]);
    vi.mocked(desktopApi.listRouteRules).mockResolvedValue([]);
    vi.mocked(desktopApi.getLifecycleStatus).mockResolvedValue(inactiveStatus);
    vi.mocked(clientFacade.loadSnapshot).mockResolvedValue(clientSnapshot);
    vi.mocked(clientFacade.getDiagnostics).mockResolvedValue([]);
    vi.mocked(clientFacade.subscribe).mockResolvedValue(vi.fn());
    vi.mocked(clientFacade.subscribeDiagnostics).mockResolvedValue(vi.fn());
    vi.mocked(desktopApi.getClientSettings).mockResolvedValue({
      autoStart: true,
      startupConsentGranted: false,
      port: 16729,
      launchAtLogin: false,
      closeToTray: true,
      language: "system",
      theme: "system",
    });
    vi.mocked(desktopApi.setClientSettings).mockImplementation(async (settings) => settings);
  });

  it("renders the desktop client toolbar and provider workspace", async () => {
    vi.mocked(clientFacade.loadSnapshot).mockResolvedValue({
      ...clientSnapshot,
      workspaces: [{
        path: "/tmp/project",
        exists: true,
        sessionId: "session-1",
        sessionIds: ["session-1"],
        threadIds: ["thread-1"],
        providerId: "provider-a",
        conflictingWorkspaces: false,
      }],
    });
    const user = userEvent.setup();
    render(<App />);

    expect(screen.getByRole("main")).toBeTruthy();
    expect(screen.getByRole("banner", { name: "Codex Route navigation" })).toBeTruthy();
    expect(document.querySelectorAll("img.brand-logo")).toHaveLength(1);
    expect(screen.getByRole("tab", { name: "Overview" }).getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tab", { name: "Providers" }).getAttribute("aria-selected")).toBe("false");
    expect(screen.queryByRole("tab", { name: "Workspace rules" })).toBeNull();
    expect(await screen.findByRole("region", { name: "Workspace routes" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Overview" }).querySelector(".nav-count")?.textContent).toBe("1");
    expect(screen.queryByRole("button", { name: "Configure routes" })).toBeNull();
    expect(screen.queryByRole("dialog", { name: "Workspace rules" })).toBeNull();
    await user.click(screen.getByRole("tab", { name: "Providers" }));
    expect(await screen.findByRole("region", { name: "Providers" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Providers" }).querySelector(".nav-count")?.textContent).toBe("1");
    expect(screen.getByRole("switch", { name: "Activate route" }).getAttribute("data-route-state")).toBe(
      "inactive",
    );
  });

  it("suppresses native context menus in the desktop shell", async () => {
    render(<App />);

    const event = createEvent.contextMenu(screen.getByRole("main"));
    fireEvent(screen.getByRole("main"), event);

    expect(event.defaultPrevented).toBe(true);
  });

  it("activates the route from the global toggle with the default port", async () => {
    const user = userEvent.setup();
    render(<App />);
    const routeToggle = await screen.findByRole("switch", { name: "Activate route" });

    await user.click(routeToggle);

    await waitFor(() => expect(desktopApi.activateRoute).toHaveBeenCalledWith(16729));
  });

  it("uses the saved Settings port when activating from the global toggle", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("tab", { name: "Settings" }));
    const port = await screen.findByRole("spinbutton", { name: "Route port" });
    await user.clear(port);
    await user.type(port, "23456");
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    await waitFor(() => expect(desktopApi.setClientSettings).toHaveBeenCalledWith({
      autoStart: true,
      startupConsentGranted: false,
      port: 23456,
      launchAtLogin: false,
      closeToTray: true,
      language: "system",
      theme: "system",
    }));
    await user.click(screen.getByRole("tab", { name: "Overview" }));
    await user.click(await screen.findByRole("switch", { name: "Activate route" }));

    await waitFor(() => expect(desktopApi.activateRoute).toHaveBeenCalledWith(23456));
  });

  it("saves the default provider and port from one Settings action", async () => {
    const providerB: ProviderSummary = {
      id: "provider-b",
      name: "Provider B",
      source: "cc-switch",
      isCurrent: false,
    };
    const snapshotWithProviders = {
      ...clientSnapshot,
      providers: [provider, providerB],
    };
    vi.mocked(clientFacade.loadSnapshot).mockResolvedValue(snapshotWithProviders);
    vi.mocked(desktopApi.setCurrentProvider).mockResolvedValue({ ...providerB, isCurrent: true });
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("tab", { name: "Settings" }));
    const providerTrigger = await screen.findByRole("button", { name: "Default route provider" });
    await waitFor(() => expect(providerTrigger).toHaveProperty("disabled", false));
    await user.click(providerTrigger);
    await user.click(within(screen.getByRole("listbox")).getByRole("option", { name: "Provider B" }));
    expect(screen.getByRole("button", { name: "Default route provider" }).textContent).toContain("Provider B");
    const port = screen.getByRole("spinbutton", { name: "Route port" });
    await user.clear(port);
    await user.type(port, "23456");
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    await waitFor(() => {
      expect(desktopApi.setCurrentProvider).toHaveBeenCalledWith("provider-b");
      expect(desktopApi.setClientSettings).toHaveBeenCalledWith({
        autoStart: true,
        startupConsentGranted: false,
        port: 23456,
        launchAtLogin: false,
        closeToTray: true,
        language: "system",
        theme: "system",
      });
    });
  });

  it("scans automatically and imports the selected providers", async () => {
    vi.mocked(desktopApi.listProviders)
      .mockResolvedValueOnce([])
      .mockResolvedValue([provider]);
    vi.mocked(desktopApi.scanCcSwitchProviders).mockResolvedValue({
      source: "/tmp/cc-switch.db",
      providers: [
        { id: "provider-a", name: "Provider A", category: "Official", alreadyImported: false },
        { id: "provider-b", name: "Provider B", alreadyImported: true },
      ],
      rejected: [],
    });
    vi.mocked(desktopApi.importCcSwitchProviders).mockResolvedValue({
      source: "/tmp/cc-switch.db",
      imported: 1,
      replaced: 0,
      renamed: 0,
      skipped: 0,
      rejected: [],
    });
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("tab", { name: "Providers" }));
    await screen.findByRole("region", { name: "Providers" });

    await user.click(screen.getByRole("button", { name: "Import providers from cc-switch" }));
    expect(await screen.findByRole("dialog", { name: "Import from cc-switch" })).toBeTruthy();
    expect(desktopApi.scanCcSwitchProviders).toHaveBeenCalledOnce();
    await user.click(screen.getByRole("checkbox", { name: /Provider B/ }));
    await user.click(screen.getByRole("button", { name: "Import selected (1)" }));

    await waitFor(() =>
      expect(desktopApi.importCcSwitchProviders).toHaveBeenCalledWith({
        providerIds: ["provider-a"],
        conflictPolicy: "skip",
      }),
    );
    expect(await screen.findByRole("button", { name: "Current" })).toBeTruthy();
    expect(screen.getByText("Imported 1 · Replaced 0 · Renamed 0 · Skipped 0 · Rejected 0")).toBeTruthy();
  });
});
