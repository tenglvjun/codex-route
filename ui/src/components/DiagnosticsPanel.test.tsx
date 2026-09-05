// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { DiagnosticsPanel } from "./DiagnosticsPanel";

afterEach(cleanup);

describe("DiagnosticsPanel", () => {
  it("filters recent diagnostics by severity", () => {
    render(<DiagnosticsPanel records={[
      { id: 1, timestamp: 1, severity: "error", code: "route.failed", message: "Failed", source: "runtime", context: {} },
      { id: 2, timestamp: 2, severity: "info", code: "route.ready", message: "Ready", source: "runtime", context: {} },
    ]} />);

    expect(screen.getByText("route.failed")).toBeTruthy();
    expect(screen.getByText("route.ready")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Filter diagnostics" }));
    fireEvent.click(screen.getByRole("listbox").querySelector('[role="option"][aria-selected="false"]')!);
    expect(screen.getByText("route.failed")).toBeTruthy();
    expect(screen.queryByText("route.ready")).toBeNull();
  });

  it("does not expose a workspace-rules navigation action", () => {
    render(<DiagnosticsPanel records={[{ id: 1, timestamp: 1, severity: "warning", code: "workspace.scan", message: "Workspace changed", source: "scanner", context: {} }]} />);

    expect(screen.queryByRole("button", { name: "Open Workspace rules" })).toBeNull();
  });
});
