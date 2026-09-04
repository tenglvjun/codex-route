// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SettingsPanel } from "./SettingsPanel";

describe("SettingsPanel", () => {
  it("changes the persisted default route provider", () => {
    const onDefaultProviderChange = vi.fn();
    render(
      <SettingsPanel
        providers={[
          { id: "provider-a", name: "Provider A", source: "local", isCurrent: true },
          { id: "provider-b", name: "Provider B", source: "cc-switch", isCurrent: false },
        ]}
        defaultProviderId="provider-a"
        busy={false}
        onDefaultProviderChange={onDefaultProviderChange}
      />,
    );

    const select = screen.getByLabelText("Default route provider") as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "provider-b" } });
    expect(onDefaultProviderChange).toHaveBeenCalledWith("provider-b");
  });
});
