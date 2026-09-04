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
    fireEvent.change(screen.getByLabelText("Filter diagnostics"), { target: { value: "error" } });
    expect(screen.getByText("route.failed")).toBeTruthy();
    expect(screen.queryByText("route.ready")).toBeNull();
  });
});
