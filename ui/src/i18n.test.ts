import { describe, expect, it } from "vitest";
import { createTranslator, resolveLocale } from "./i18n";

describe("i18n", () => {
  it("resolves explicit and system language preferences", () => {
    expect(resolveLocale("zh-CN", "en-US")).toBe("zh-CN");
    expect(resolveLocale("zh-TW", "en-US")).toBe("zh-TW");
    expect(resolveLocale("en", "zh-CN")).toBe("en");
    expect(resolveLocale("system", "zh-HK")).toBe("zh-TW");
    expect(resolveLocale("system", "zh-CN")).toBe("zh-CN");
    expect(resolveLocale("system", "de-DE")).toBe("en");
  });

  it("interpolates translated copy", () => {
    expect(createTranslator("zh-CN")("selectedCount", { count: 3 })).toBe("已选择 3 项");
    expect(createTranslator("zh-TW")("newSessionsUse", { provider: "Provider A" })).toContain("Provider A");
  });
});
