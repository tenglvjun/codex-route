// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SettingsPanel } from "./SettingsPanel";

describe("SettingsPanel", () => {
  afterEach(cleanup);

  it("stages both settings and saves them together", () => {
    const onDefaultProviderChange = vi.fn();
    const onSave = vi.fn();
    render(
      <SettingsPanel
        providers={[
          { id: "provider-a", name: "Provider A", source: "local", isCurrent: true },
          { id: "provider-b", name: "Provider B", source: "cc-switch", isCurrent: false },
        ]}
        settings={{ autoStart: true, startupConsentGranted: false, port: 16729, launchAtLogin: false, closeToTray: true, language: "system", theme: "system" }}
        defaultProviderId="provider-a"
        port="16729"
        busy={false}
        onDefaultProviderChange={onDefaultProviderChange}
        onPortChange={vi.fn()}
        onSave={onSave}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Default route provider" }));
    expect(screen.queryByRole("combobox")).toBeNull();
    fireEvent.click(screen.getByRole("listbox").querySelector('[role="option"][aria-selected="false"]')!);
    expect(onDefaultProviderChange).toHaveBeenCalledWith("provider-b");
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    expect(onSave).toHaveBeenCalledWith({ providerId: "provider-b", port: 16729, launchAtLogin: false, closeToTray: true, language: "system", theme: "system" });
  });

  it("groups settings under a single page heading", () => {
    const { container } = render(
      <SettingsPanel
        providers={[]}
        settings={{ autoStart: true, startupConsentGranted: false, port: 16729, launchAtLogin: false, closeToTray: true, language: "system", theme: "system" }}
        port="16729"
        busy={false}
        onDefaultProviderChange={vi.fn()}
        onPortChange={vi.fn()}
        onSave={vi.fn()}
      />,
    );

    expect(container.querySelector(".panel-heading")).toBeNull();
    expect(container.querySelector(".settings-list .settings-row")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Settings", level: 1 })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Startup behavior", level: 2 })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Interface preferences", level: 2 })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Route settings", level: 2 })).toBeTruthy();
    expect(container.querySelectorAll(".settings-section")).toHaveLength(3);
  });

  it("rejects an invalid route port before saving", () => {
    const onSave = vi.fn();
    render(
      <SettingsPanel
        providers={[]}
        settings={{ autoStart: true, startupConsentGranted: false, port: 16729, launchAtLogin: false, closeToTray: true, language: "system", theme: "system" }}
        port="0"
        busy={false}
        onDefaultProviderChange={vi.fn()}
        onPortChange={vi.fn()}
        onSave={onSave}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    expect(screen.getByRole("alert").textContent).toContain("between 1 and 65535");
    expect(onSave).not.toHaveBeenCalled();
  });

  it("saves desktop preferences with the provider and port as one draft", () => {
    const onSave = vi.fn();
    render(
      <SettingsPanel
        providers={[{ id: "provider-a", name: "Provider A", source: "local", isCurrent: true }]}
        settings={{ autoStart: true, startupConsentGranted: false, port: 16729, launchAtLogin: false, closeToTray: true, language: "system", theme: "system" }}
        defaultProviderId="provider-a"
        port="16729"
        busy={false}
        onDefaultProviderChange={vi.fn()}
        onPortChange={vi.fn()}
        onSave={onSave}
      />,
    );

    fireEvent.click(screen.getByRole("switch", { name: "Launch at login" }));
    fireEvent.click(screen.getByRole("switch", { name: "Close window to tray" }));
    fireEvent.click(screen.getByRole("button", { name: "Language" }));
    fireEvent.click(screen.getByRole("option", { name: "繁體中文" }));
    fireEvent.click(screen.getByRole("button", { name: "Theme" }));
    fireEvent.click(screen.getByRole("option", { name: "Dark" }));
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));

    expect(onSave).toHaveBeenCalledWith({
      providerId: "provider-a",
      port: 16729,
      launchAtLogin: true,
      closeToTray: false,
      language: "zh-TW",
      theme: "dark",
    });
  });
});
