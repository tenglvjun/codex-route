// @vitest-environment jsdom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { desktopApi, type LifecycleStatus, type ProviderSummary } from "./api";
import App from "./App";

vi.mock("./api", () => ({
  desktopApi: {
    listProviders: vi.fn(),
    listRouteRules: vi.fn(),
    getLifecycleStatus: vi.fn(),
    setCurrentProvider: vi.fn(),
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
});
