// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
    sessionIds: ["session-1"],
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

  it("shows every workspace route and runtime together", () => {
    render(
      <DashboardPage
        snapshot={snapshot}
        onProviderChange={vi.fn()}
        onStopRuntime={vi.fn()}
        workspaceRulesOpen={false}
        onToggleWorkspaceRules={vi.fn()}
      />,
    );
    expect(screen.getByRole("heading", { name: "Workspace routes" })).toBeTruthy();
    expect((screen.getByLabelText("Route for /tmp/project") as HTMLSelectElement).value).toBe("provider-a");
    expect(screen.getByText("Ready")).toBeTruthy();
    expect(screen.getByText("127.0.0.1:16729")).toBeTruthy();
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
        workspaceRulesOpen={false}
        onToggleWorkspaceRules={vi.fn()}
      />,
    );

    const route = screen.getByLabelText("Route for /tmp/other-project") as HTMLSelectElement;
    expect(route.value).toBe("");
    expect(route.options[0].textContent).toContain("Provider A");
    fireEvent.change(route, { target: { value: "provider-b" } });
    expect(onProviderChange).toHaveBeenCalledWith("/tmp/other-project", "provider-b");
  });

  it("exposes the route settings disclosure", () => {
    const onToggleWorkspaceRules = vi.fn();
    render(
      <DashboardPage
        snapshot={snapshot}
        workspaceRulesOpen={false}
        onToggleWorkspaceRules={onToggleWorkspaceRules}
      />,
    );

    const toggle = screen.getByRole("button", { name: "Configure routes" });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(toggle.getAttribute("aria-controls")).toBe("workspace-rules-dialog");
    fireEvent.click(toggle);
    expect(onToggleWorkspaceRules).toHaveBeenCalledOnce();
  });
});
