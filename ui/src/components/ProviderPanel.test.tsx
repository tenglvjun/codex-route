// @vitest-environment jsdom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CcSwitchScanReport, ImportReport } from "../api";
import { ProviderPanel } from "./ProviderPanel";

const report: ImportReport = {
  source: "/tmp/cc-switch.db",
  imported: 2,
  replaced: 1,
  renamed: 0,
  skipped: 3,
  rejected: [{ id: "invalid", reason: "missing base_url" }],
};

const scanReport: CcSwitchScanReport = {
  source: "/tmp/cc-switch.db",
  providers: [
    { id: "provider-a", name: "Provider A", category: "Official", alreadyImported: false },
    { id: "provider-b", name: "Provider B", alreadyImported: true },
  ],
  rejected: [{ id: "invalid", reason: "missing base_url" }],
};

afterEach(cleanup);

describe("ProviderPanel", () => {
  beforeEach(() => vi.clearAllMocks());

  it("scans on open and selects every importable provider by default", async () => {
    const onScan = vi.fn().mockResolvedValue(scanReport);
    const onImport = vi.fn().mockResolvedValue(report);
    render(
      <ProviderPanel
        providers={[]}
        busy={false}
        onSelect={vi.fn()}
        onScan={onScan}
        onImport={onImport}
        importOpen
        onImportOpenChange={vi.fn()}
      />,
    );

    expect(await screen.findByRole("dialog", { name: "Import from cc-switch" })).toBeTruthy();
    expect(onScan).toHaveBeenCalledOnce();
    const providerCheckboxes = screen.getAllByRole("checkbox", { name: /Provider [AB]/ });
    expect(providerCheckboxes).toHaveLength(2);
    expect(providerCheckboxes.every((checkbox) => (checkbox as HTMLInputElement).checked)).toBe(true);
    expect(screen.getByText("2 selected")).toBeTruthy();
    expect(onImport).not.toHaveBeenCalled();
  });

  it("imports only the checked providers with the chosen conflict policy", async () => {
    const onImport = vi.fn().mockResolvedValue(report);
    const user = userEvent.setup();
    render(
      <ProviderPanel
        providers={[]}
        busy={false}
        onSelect={vi.fn()}
        onScan={vi.fn().mockResolvedValue(scanReport)}
        onImport={onImport}
        importOpen
        onImportOpenChange={vi.fn()}
      />,
    );

    await screen.findByRole("checkbox", { name: /Provider A/ });
    await user.click(screen.getByRole("checkbox", { name: /Provider B/ }));
    await user.selectOptions(screen.getByLabelText("On conflict"), "replace");
    await user.click(screen.getByRole("button", { name: "Import selected (1)" }));

    await waitFor(() =>
      expect(onImport).toHaveBeenCalledWith({
        providerIds: ["provider-a"],
        conflictPolicy: "replace",
      }),
    );
    expect(await screen.findByText("Import complete")).toBeTruthy();
    expect(screen.getByText("Imported 2 · Replaced 1 · Renamed 0 · Skipped 3 · Rejected 1")).toBeTruthy();
  });

  it("shows scan failures in the dialog and retries", async () => {
    const onScan = vi
      .fn()
      .mockRejectedValueOnce(new Error("cc-switch database was not found"))
      .mockResolvedValue(scanReport);
    const user = userEvent.setup();
    render(
      <ProviderPanel
        providers={[]}
        busy={false}
        onSelect={vi.fn()}
        onScan={onScan}
        onImport={vi.fn()}
        importOpen
        onImportOpenChange={vi.fn()}
      />,
    );

    expect((await screen.findByRole("alert")).textContent).toContain("cc-switch database was not found");
    await user.click(screen.getByRole("button", { name: "Retry scan" }));
    expect(await screen.findByRole("checkbox", { name: /Provider A/ })).toBeTruthy();
    expect(onScan).toHaveBeenCalledTimes(2);
  });

  it("uses the controlled import button to open the import dialog", async () => {
    const onImportOpenChange = vi.fn();
    const user = userEvent.setup();
    render(
      <ProviderPanel
        providers={[]}
        busy={false}
        onSelect={vi.fn()}
        onScan={vi.fn().mockResolvedValue(scanReport)}
        onImport={vi.fn()}
        importOpen={false}
        onImportOpenChange={onImportOpenChange}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Import cc-switch" }));

    expect(onImportOpenChange).toHaveBeenCalledWith(true);
  });

  it("shows row-shaped placeholders while providers are loading", () => {
    render(
      <ProviderPanel
        providers={[]}
        busy={true}
        loading
        onSelect={vi.fn()}
        onScan={vi.fn()}
        onImport={vi.fn()}
      />,
    );

    const loadingState = screen.getByRole("status", { name: "Loading providers" });
    expect(loadingState.querySelectorAll(".provider-skeleton")).toHaveLength(3);
    expect(screen.queryByText("No providers yet")).toBeNull();
  });

  it("marks the current provider and selects a different provider", async () => {
    const onSelect = vi.fn();
    const user = userEvent.setup();
    const { container } = render(
      <ProviderPanel
        providers={[
          { id: "provider-a", name: "Provider A", source: "local", isCurrent: true },
          { id: "provider-b", name: "Provider B", source: "imported", isCurrent: false },
        ]}
        busy={false}
        onSelect={onSelect}
        onScan={vi.fn()}
        onImport={vi.fn()}
      />,
    );

    const currentRow = container.querySelector('[data-provider-id="provider-a"]');
    expect(currentRow?.querySelector(".provider-status-dot.active")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Current" })).toHaveProperty("disabled", true);

    await user.click(screen.getByRole("button", { name: "Use provider" }));

    expect(onSelect).toHaveBeenCalledWith("provider-b");
  });
});
