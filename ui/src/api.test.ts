import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { desktopApi } from "./api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("desktopApi", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it("uses the command payload shapes expected by Tauri", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await desktopApi.upsertRouteRule({
      workspace: "/tmp/project",
      providerId: "provider-a",
      replace: true,
    });
    await desktopApi.removeRouteRule("/tmp/project");
    await desktopApi.activateRoute(16729);
    await desktopApi.importCcSwitchProviders({
      databasePath: "/tmp/cc-switch.db",
      conflictPolicy: "replace",
    });

    expect(invoke).toHaveBeenNthCalledWith(1, "upsert_route_rule", {
      request: { workspace: "/tmp/project", providerId: "provider-a", replace: true },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "remove_route_rule", {
      workspace: "/tmp/project",
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "activate_route", {
      request: { port: 16729 },
    });
    expect(invoke).toHaveBeenNthCalledWith(4, "import_cc_switch_providers", {
      request: {
        databasePath: "/tmp/cc-switch.db",
        conflictPolicy: "replace",
      },
    });
  });
});
