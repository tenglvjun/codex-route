// @vitest-environment jsdom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { open } from "@tauri-apps/plugin-dialog";
import { desktopApi, type LifecycleStatus, type ProviderSummary } from "./api";
import App from "./App";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

vi.mock("./api", () => ({
  desktopApi: {
    listProviders: vi.fn(),
    listRouteRules: vi.fn(),
    getLifecycleStatus: vi.fn(),
    setCurrentProvider: vi.fn(),
    importCcSwitchProviders: vi.fn(),
    upsertRouteRule: vi.fn(),
    removeRouteRule: vi.fn(),
    activateRoute: vi.fn(),
    deactivateRoute: vi.fn(),
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

afterEach(cleanup);

describe("App", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(desktopApi.listProviders).mockResolvedValue([provider]);
    vi.mocked(desktopApi.listRouteRules).mockResolvedValue([]);
    vi.mocked(desktopApi.getLifecycleStatus).mockResolvedValue(inactiveStatus);
  });

  it("renders the desktop client toolbar and provider workspace", async () => {
    render(<App />);

    expect(screen.getByRole("main")).toBeTruthy();
    expect(screen.getByRole("banner", { name: "Codex Route navigation" })).toBeTruthy();
    expect(document.querySelectorAll("img.brand-logo")).toHaveLength(2);
    expect(screen.getByRole("tab", { name: "Providers" }).getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tab", { name: "Workspace rules" }).getAttribute("aria-selected")).toBe("false");
    expect(await screen.findByRole("heading", { name: "Default fallback provider" })).toBeTruthy();
    expect(screen.getByRole("switch", { name: "Activate route" }).getAttribute("data-route-state")).toBe(
      "inactive",
    );
  });

  it("rejects an invalid route port before invoking Tauri", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: "Default fallback provider" });

    const port = screen.getByLabelText("Port");
    await user.clear(port);
    await user.type(port, "0");
    await user.click(screen.getByRole("button", { name: "Activate" }));

    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("between 1 and 65535"));
    expect(desktopApi.activateRoute).not.toHaveBeenCalled();
  });

  it("imports providers and refreshes the provider list", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/cc-switch.db");
    vi.mocked(desktopApi.listProviders)
      .mockResolvedValueOnce([])
      .mockResolvedValue([provider]);
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
    await screen.findByRole("heading", { name: "Default fallback provider" });

    await user.click(screen.getByRole("button", { name: "Import providers" }));
    await user.click(screen.getByRole("button", { name: "Choose database" }));

    await waitFor(() =>
      expect(desktopApi.importCcSwitchProviders).toHaveBeenCalledWith({
        databasePath: "/tmp/cc-switch.db",
        conflictPolicy: "skip",
      }),
    );
    expect(await screen.findByRole("button", { name: "Current" })).toBeTruthy();
    expect(screen.getByText("Imported 1 · Replaced 0 · Renamed 0 · Skipped 0 · Rejected 0")).toBeTruthy();
  });
});
