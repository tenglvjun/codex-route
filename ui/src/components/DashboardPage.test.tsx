// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ClientSnapshot } from "../api";
import { DashboardPage } from "./DashboardPage";

const snapshot: ClientSnapshot = {
    schemaVersion: 1,
    sequence: 1,
  generatedAt: 1,
  codex: {
    home: "/tmp/.codex",
    configPath: "/tmp/.codex/config.toml",
    installed: true,
    configExists: true,
    configManaged: true,
    externalModification: false,
  },
  workspaces: [{
    path: "/tmp/project",
    exists: true,
    sessionId: "session-1",
    sessionIds: ["session-1", "session-2", "session-3", "session-4"],
    threadIds: ["thread-1"],
    providerId: "provider-a",
    conflictingWorkspaces: false,
  }],
  workspace: undefined,
  provider: { id: "provider-a", name: "Provider A", source: "local", isCurrent: true },
  providers: [
    { id: "provider-a", name: "Provider A", source: "local", isCurrent: true },
    { id: "provider-b", name: "Provider B", source: "cc-switch", isCurrent: false },
  ],
  rules: [],
  runtime: {
    phase: "running",
    active: true,
    port: 16729,
    serverReachable: true,
    configManaged: true,
    externalModification: false,
    restartCount: 0,
    updatedAt: 1,
    sequence: 1,
  },
  diagnostics: { unreadCount: 0 },
};

describe("DashboardPage", () => {
  afterEach(cleanup);

  it("shows every workspace route without duplicating runtime controls", () => {
    render(
      <DashboardPage
        snapshot={snapshot}
        onProviderChange={vi.fn()}
      />,
    );
    const overview = screen.getByRole("region", { name: "Workspace routes" });
    expect(overview.classList.contains("utility-section")).toBe(true);
    expect(overview.querySelector(".panel.overview-panel")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Workspaces", level: 1 })).toBeTruthy();
    expect(screen.getByText("Manage and configure your routing workspaces.")).toBeTruthy();
    expect(screen.getByLabelText("1 active workspace").textContent).toBe("1");
    expect(overview.querySelector(".workspace-route-toolbar")).toBeNull();
    expect(overview.querySelector(".panel-heading")).toBeNull();
    expect(screen.queryByText("OVERVIEW")).toBeNull();
    expect(screen.queryByText("WORKSPACE")).toBeNull();
    expect(screen.queryByText("PROVIDER ROUTE")).toBeNull();
    expect(screen.getByRole("button", { name: "Select provider route for /tmp/project" }).textContent).toContain("Provider A");
    expect(overview.querySelector(".workspace-route-title")?.textContent).toBe("project (4)");
    expect(screen.getByText("/tmp/project", { selector: ".workspace-route-path" })).toBeTruthy();
    expect(screen.getByLabelText("4 sessions")).toBeTruthy();
    expect(overview.querySelectorAll(".workspace-route-row")).toHaveLength(1);
    expect(overview.querySelector(".workspace-route-row")?.getAttribute("role")).toBe("listitem");
    expect(screen.queryByLabelText("1 thread")).toBeNull();
    expect(overview.querySelector(".workspace-route-metric")).toBeNull();
    expect(screen.queryByText("Ready")).toBeNull();
    expect(screen.queryByText("127.0.0.1:16729")).toBeNull();
  });

  it("uses the themed provider menu to change a workspace route", () => {
    const onProviderChange = vi.fn();
    render(
      <DashboardPage
        snapshot={snapshot}
        onProviderChange={onProviderChange}
      />,
    );

    const trigger = screen.getByRole("button", { name: "Select provider route for /tmp/project" });
    fireEvent.click(trigger);
    expect(trigger.getAttribute("aria-expanded")).toBe("true");
    fireEvent.click(within(screen.getByRole("listbox")).getByRole("option", { name: "Provider B" }));

    expect(onProviderChange).toHaveBeenCalledWith("/tmp/project", "provider-b");
    expect(trigger.textContent).toContain("Provider B");
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
  });

  it("exposes a default fallback for unconfigured workspaces", () => {
    const onProviderChange = vi.fn();
    const multiWorkspaceSnapshot = {
      ...snapshot,
      workspaces: [
        ...snapshot.workspaces,
        {
          path: "/tmp/other-project",
          exists: true,
          sessionId: "session-2",
          sessionIds: ["session-2"],
          threadIds: ["thread-2"],
          conflictingWorkspaces: false,
        },
      ],
    };
    render(
      <DashboardPage
        snapshot={multiWorkspaceSnapshot}
        onProviderChange={onProviderChange}
      />,
    );

    const route = screen.getByRole("button", { name: "Select provider route for /tmp/other-project" });
    expect(route.textContent).toContain("Use default");
    fireEvent.click(route);
    fireEvent.click(within(screen.getByRole("listbox")).getByRole("option", { name: "Provider B" }));
    expect(onProviderChange).toHaveBeenCalledWith("/tmp/other-project", "provider-b");
  });

});
