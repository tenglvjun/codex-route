// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from "vitest";
import { applyTheme, resolveTheme } from "./theme";

afterEach(() => {
  document.documentElement.removeAttribute("data-theme");
  document.documentElement.style.removeProperty("color-scheme");
  vi.restoreAllMocks();
});

describe("theme", () => {
  it("resolves system appearance", () => {
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
  });

  it("applies an explicit theme without installing a listener", () => {
    const cleanup = applyTheme("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.style.colorScheme).toBe("dark");
    cleanup();
  });
});
