// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { LifecycleStatus } from "../api";
import { RouteStatusPanel } from "./RouteStatusPanel";

afterEach(cleanup);

const activeStatus: LifecycleStatus = {
  status: "active",
  active: true,
  pid: 1,
  port: 16729,
  serverReachable: true,
  configManaged: true,
  externalModification: false,
  configPath: "/tmp/config.toml",
  statePath: "/tmp/route-state.json",
  lockPath: "/tmp/route.lock",
};

describe("RouteStatusPanel", () => {
  it("shows the active route URL and deactivates", async () => {
    const onDeactivate = vi.fn();
    const user = userEvent.setup();
    render(
      <RouteStatusPanel
        status={activeStatus}
        port="16729"
        busy={false}
        canActivate={true}
        onPortChange={vi.fn()}
        onActivate={vi.fn()}
        onDeactivate={onDeactivate}
      />,
    );

    expect(screen.getByText("http://127.0.0.1:16729/v1")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Deactivate" }));
    expect(onDeactivate).toHaveBeenCalledOnce();
  });
});
