// @vitest-environment jsdom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { confirm } from "@tauri-apps/plugin-dialog";
import type { ProviderSummary, WorkspaceRouteRule } from "../api";
import { WorkspaceRulesPanel } from "./WorkspaceRulesPanel";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(),
  open: vi.fn(),
}));

const providers: ProviderSummary[] = [
  { id: "provider-a", name: "Provider A", source: "local", isCurrent: true },
  { id: "provider-b", name: "Provider B", source: "local", isCurrent: false },
];

const rule: WorkspaceRouteRule = {
  workspace: "/tmp/project",
  providerId: "provider-a",
  createdAt: 1,
  updatedAt: 1,
};

afterEach(cleanup);

describe("WorkspaceRulesPanel", () => {
  beforeEach(() => vi.clearAllMocks());

  it("creates a workspace rule with the current provider", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(
      <WorkspaceRulesPanel
        providers={providers}
        rules={[]}
        busy={false}
        onSave={onSave}
        onRemove={vi.fn()}
        onError={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Add rule" }));
    await user.type(screen.getByLabelText("Workspace path"), "/tmp/project");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith({
        workspace: "/tmp/project",
        providerId: "provider-a",
        replace: false,
      }),
    );
  });

  it("edits the provider while preserving workspace identity", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(
      <WorkspaceRulesPanel
        providers={providers}
        rules={[rule]}
        busy={false}
        onSave={onSave}
        onRemove={vi.fn()}
        onError={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: `Edit route for ${rule.workspace}` }));
    expect(screen.getByLabelText("Workspace path")).toHaveProperty("readOnly", true);
    await user.selectOptions(screen.getByLabelText("Provider"), "provider-b");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith({
        workspace: rule.workspace,
        providerId: "provider-b",
        replace: true,
      }),
    );
  });

  it("keeps a rule when removal is cancelled", async () => {
    vi.mocked(confirm).mockResolvedValue(false);
    const onRemove = vi.fn().mockResolvedValue(true);
    const user = userEvent.setup();
    render(
      <WorkspaceRulesPanel
        providers={providers}
        rules={[rule]}
        busy={false}
        onSave={vi.fn()}
        onRemove={onRemove}
        onError={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: `Remove route for ${rule.workspace}` }));

    expect(confirm).toHaveBeenCalledOnce();
    expect(onRemove).not.toHaveBeenCalled();
  });

  it("surfaces removal failures without an unhandled rejection", async () => {
    vi.mocked(confirm).mockResolvedValue(true);
    const onRemove = vi.fn().mockRejectedValue(new Error("database is locked"));
    const onError = vi.fn();
    const user = userEvent.setup();
    render(
      <WorkspaceRulesPanel
        providers={providers}
        rules={[rule]}
        busy={false}
        onSave={vi.fn()}
        onRemove={onRemove}
        onError={onError}
      />,
    );

    await user.click(screen.getByRole("button", { name: `Remove route for ${rule.workspace}` }));

    await waitFor(() => expect(onError).toHaveBeenCalledWith("database is locked"));
  });

  it("shows a field error for an empty workspace", async () => {
    const onSave = vi.fn();
    const user = userEvent.setup();
    render(
      <WorkspaceRulesPanel
        providers={providers}
        rules={[]}
        busy={false}
        onSave={onSave}
        onRemove={vi.fn()}
        onError={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Add rule" }));
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(screen.getByText("Workspace path is required.")).toBeTruthy();
    expect(onSave).not.toHaveBeenCalled();
  });

  it("collapses the form when cancelled", async () => {
    const user = userEvent.setup();
    render(
      <WorkspaceRulesPanel
        providers={providers}
        rules={[]}
        busy={false}
        onSave={vi.fn()}
        onRemove={vi.fn()}
        onError={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Add rule" }));
    expect(screen.getByLabelText("Workspace path")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(screen.queryByLabelText("Workspace path")).toBeNull();
  });

  it("keeps the form populated when the backend rejects a rule", async () => {
    const onSave = vi.fn().mockRejectedValue(new Error("workspace path must be absolute"));
    const user = userEvent.setup();
    render(
      <WorkspaceRulesPanel
        providers={providers}
        rules={[]}
        busy={false}
        onSave={onSave}
        onRemove={vi.fn()}
        onError={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Add rule" }));
    await user.type(screen.getByLabelText("Workspace path"), "relative/project");
    await user.click(screen.getByRole("button", { name: "Save" }));

    const error = await screen.findByRole("alert");
    expect(error.textContent).toContain("workspace path must be absolute");
    expect(screen.getByLabelText("Workspace path")).toHaveProperty("value", "relative/project");
  });
});
