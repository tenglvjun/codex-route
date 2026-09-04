// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
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
  workspace: {
    path: "/tmp/project",
    exists: true,
    sessionId: "session-1",
    threadIds: ["thread-1"],
    providerId: "provider-a",
    conflictingWorkspaces: false,
  },
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
  it("shows the current workspace, provider and runtime together", () => {
    render(<DashboardPage snapshot={snapshot} onProviderChange={vi.fn()} onStopRuntime={vi.fn()} />);
    expect(screen.getByRole("heading", { name: "/tmp/project" })).toBeTruthy();
    expect((screen.getByLabelText("Active for this workspace") as HTMLSelectElement).value).toBe("provider-a");
    expect(screen.getByRole("status").textContent).toContain("running");
    expect(screen.getByText("Listening on 127.0.0.1:16729")).toBeTruthy();
  });
});
