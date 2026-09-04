// @vitest-environment jsdom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { open } from "@tauri-apps/plugin-dialog";
import type { ImportReport } from "../api";
import { ProviderPanel } from "./ProviderPanel";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

const report: ImportReport = {
  source: "/tmp/cc-switch.db",
  imported: 2,
  replaced: 1,
  renamed: 0,
  skipped: 3,
  rejected: [{ id: "invalid", reason: "missing base_url" }],
};

afterEach(cleanup);

describe("ProviderPanel", () => {
  beforeEach(() => vi.clearAllMocks());

  it("imports a selected cc-switch database with the chosen conflict policy", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/cc-switch.db");
    const onImport = vi.fn().mockResolvedValue(report);
    const user = userEvent.setup();
    render(
      <ProviderPanel
        providers={[]}
        busy={false}
        onSelect={vi.fn()}
        onImport={onImport}
        onError={vi.fn()}
      />,
    );

    await user.selectOptions(screen.getByLabelText("On conflict"), "replace");
    await user.click(screen.getByRole("button", { name: "Import cc-switch" }));

    await waitFor(() =>
      expect(onImport).toHaveBeenCalledWith({
        databasePath: "/tmp/cc-switch.db",
        conflictPolicy: "replace",
      }),
    );
    expect(await screen.findByText("Import complete")).toBeTruthy();
    expect(screen.getByText("Imported 2 · Replaced 1 · Renamed 0 · Skipped 3 · Rejected 1")).toBeTruthy();
    expect(screen.getByText("invalid: missing base_url")).toBeTruthy();
  });

  it("does nothing when database selection is cancelled", async () => {
    vi.mocked(open).mockResolvedValue(null);
    const onImport = vi.fn();
    const user = userEvent.setup();
    render(
      <ProviderPanel
        providers={[]}
        busy={false}
        onSelect={vi.fn()}
        onImport={onImport}
        onError={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Import cc-switch" }));

    expect(open).toHaveBeenCalledOnce();
    expect(onImport).not.toHaveBeenCalled();
  });

  it("surfaces file picker errors", async () => {
    vi.mocked(open).mockRejectedValue(new Error("dialog unavailable"));
    const onError = vi.fn();
    const user = userEvent.setup();
    render(
      <ProviderPanel
        providers={[]}
        busy={false}
        onSelect={vi.fn()}
        onImport={vi.fn()}
        onError={onError}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Import cc-switch" }));

    await waitFor(() => expect(onError).toHaveBeenCalledWith("dialog unavailable"));
  });

  it("surfaces import failures and resets the busy state", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/cc-switch.db");
    const onImport = vi.fn().mockRejectedValue(new Error("database is locked"));
    const onError = vi.fn();
    const user = userEvent.setup();
    render(
      <ProviderPanel
        providers={[]}
        busy={false}
        onSelect={vi.fn()}
        onImport={onImport}
        onError={onError}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Import cc-switch" }));

    await waitFor(() => expect(onError).toHaveBeenCalledWith("database is locked"));
    expect(screen.getByRole("button", { name: "Import cc-switch" })).toHaveProperty("disabled", false);
  });

  it("uses the controlled import button to toggle the compact import panel", async () => {
    const onImportOpenChange = vi.fn();
    const user = userEvent.setup();
    const props = {
      providers: [],
      busy: false,
      onSelect: vi.fn(),
      onImport: vi.fn(),
      onError: vi.fn(),
      importOpen: false,
      onImportOpenChange,
    };
    const { rerender } = render(<ProviderPanel {...props} />);

    const importButton = screen.getByRole("button", { name: "Import cc-switch" });
    expect(importButton.getAttribute("aria-expanded")).toBe("false");
    await user.click(importButton);

    expect(onImportOpenChange).toHaveBeenCalledWith(true);
    expect(open).not.toHaveBeenCalled();

    rerender(<ProviderPanel {...props} importOpen={true} />);
    expect(screen.getByLabelText("On conflict")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Choose database" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Import cc-switch" }));
    expect(onImportOpenChange).toHaveBeenLastCalledWith(false);
  });

  it("imports from the compact controlled import panel", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/cc-switch.db");
    const onImport = vi.fn().mockResolvedValue(report);
    const user = userEvent.setup();
    render(
      <ProviderPanel
        providers={[]}
        busy={false}
        onSelect={vi.fn()}
        onImport={onImport}
        onError={vi.fn()}
        importOpen
        onImportOpenChange={vi.fn()}
      />,
    );

    await user.selectOptions(screen.getByLabelText("On conflict"), "rename");
    await user.click(screen.getByRole("button", { name: "Choose database" }));

    await waitFor(() =>
      expect(onImport).toHaveBeenCalledWith({
        databasePath: "/tmp/cc-switch.db",
        conflictPolicy: "rename",
      }),
    );
    expect(await screen.findByText("Import complete")).toBeTruthy();
  });

  it("shows row-shaped placeholders while providers are loading", () => {
    render(
      <ProviderPanel
        providers={[]}
        busy={true}
        loading
        onSelect={vi.fn()}
        onImport={vi.fn()}
        onError={vi.fn()}
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
        onImport={vi.fn()}
        onError={vi.fn()}
      />,
    );

    const currentRow = container.querySelector('[data-provider-id="provider-a"]');
    expect(currentRow?.querySelector(".provider-status-dot.active")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Current" })).toHaveProperty("disabled", true);

    await user.click(screen.getByRole("button", { name: "Use provider" }));

    expect(onSelect).toHaveBeenCalledWith("provider-b");
  });
});
