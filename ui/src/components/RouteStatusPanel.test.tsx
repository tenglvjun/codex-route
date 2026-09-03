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

const externallyModifiedStatus: LifecycleStatus = {
  ...activeStatus,
  status: "external_modified",
  externalModification: true,
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

    const panel = screen.getByRole("region", { name: "Route status" });
    expect(panel.classList.contains("route-control-strip")).toBe(true);
    expect(panel.classList.contains("route-control-strip--active")).toBe(true);
    expect(panel.getAttribute("data-route-state")).toBe("active");
    expect(screen.getByRole("group", { name: "Route controls" }).getAttribute("data-route-state")).toBe(
      "active",
    );
    expect(screen.getByText("http://127.0.0.1:16729/v1")).toBeTruthy();
    expect(screen.getByText("Managed by Codex Route")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Deactivate" }));
    expect(onDeactivate).toHaveBeenCalledOnce();
  });

  it("keeps activation available when the route is inactive", async () => {
    const onActivate = vi.fn();
    const user = userEvent.setup();
    render(
      <RouteStatusPanel
        status={inactiveStatus}
        port="16729"
        busy={false}
        canActivate={true}
        onPortChange={vi.fn()}
        onActivate={onActivate}
        onDeactivate={vi.fn()}
      />,
    );

    const panel = screen.getByRole("region", { name: "Route status" });
    expect(panel.getAttribute("data-state")).toBe("inactive");
    expect(screen.getByRole("status").textContent).toContain("Inactive");
    expect(screen.getByText("No listener")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Activate" })).toHaveProperty("disabled", false);

    await user.click(screen.getByRole("button", { name: "Activate" }));
    expect(onActivate).toHaveBeenCalledOnce();
  });

  it("warns when the route configuration was modified externally", () => {
    render(
      <RouteStatusPanel
        status={externallyModifiedStatus}
        port="16729"
        busy={false}
        canActivate={true}
        onPortChange={vi.fn()}
        onActivate={vi.fn()}
        onDeactivate={vi.fn()}
      />,
    );

    const panel = screen.getByRole("region", { name: "Route status" });
    expect(panel.classList.contains("route-control-strip--external-modified")).toBe(true);
    expect(screen.getByRole("status").textContent).toContain("External modification");
    expect(screen.getByRole("alert").textContent).toContain(
      "Codex config changed outside Codex Route. Deactivation is blocked to protect it.",
    );
  });
});
