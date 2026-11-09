import { afterEach, describe, expect, it, vi } from "vitest";
import { formatDateTime, getLocalLanguagePreference, language, languagePreference, normalizeLanguage, normalizeLanguagePreference, saveLocalLanguagePreference, setHostLanguage, setLanguage, t, useI18n } from "./index";

describe("i18n", () => {
  afterEach(() => {
    setLanguage("zh-CN");
    localStorage.clear();
  });

  it.each(["en", "en-US", "en-GB", "en_US", "fr-FR", null])("uses English for %s", (value) => {
    expect(normalizeLanguage(value)).toBe("en-US");
  });

  it.each(["zh", "zh-CN", "zh_CN", "ZH-cn"])("recognizes Chinese locale %s", (value) => {
    expect(normalizeLanguage(value)).toBe("zh-CN");
  });

  it("keeps automatic preferences distinct from resolved languages", async () => {
    expect(normalizeLanguagePreference(null)).toBe("auto");
    expect(normalizeLanguagePreference("invalid")).toBe("auto");
    setLanguage("auto");
    setHostLanguage("en");
    expect(languagePreference.value).toBe("auto");
    expect(language.value).toBe("en-US");
    await vi.waitFor(() => expect(document.documentElement.lang).toBe("en-US"));
    setLanguage("zh-CN");
    setHostLanguage("en-GB");
    expect(language.value).toBe("zh-CN");
    setLanguage("auto");
    expect(language.value).toBe("en-US");
  });

  it("clears the login override when returning to automatic language", () => {
    saveLocalLanguagePreference("zh-CN");
    expect(getLocalLanguagePreference()).toBe("zh-CN");
    saveLocalLanguagePreference("auto");
    expect(getLocalLanguagePreference()).toBeNull();
  });

  it("allows language selection when browser storage is unavailable", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw new Error("blocked"); });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("blocked"); });
    expect(getLocalLanguagePreference()).toBeNull();
    expect(() => saveLocalLanguagePreference("en-US")).not.toThrow();
  });

  it.each(["zh-CN", "en-GB", "de-DE"])("initializes standalone language from browser %s", async (locale) => {
    vi.resetModules();
    vi.stubGlobal("navigator", { language: locale });
    const i18n = await import("./index");
    expect(i18n.languagePreference.value).toBe("auto");
    expect(i18n.language.value).toBe(locale === "zh-CN" ? "zh-CN" : "en-US");
  });

  it("switches dictionaries and interpolates every matching parameter", () => {
    setLanguage("en-US");
    expect(language.value).toBe("en-US");
    expect(t("create.url.detected", { count: 2 })).toBe(
      "Detected 2 link(s); 2 separate download task(s) will be created.",
    );
    expect(useI18n().t("common.save")).toBe("Save");
    expect(useI18n().t("common.clipboardManualCopy")).toContain("LAN HTTP");
  });

  it("formats missing and valid timestamps with the active language", () => {
    expect(formatDateTime(0)).toBe("--");
    setLanguage("en-US");
    expect(formatDateTime(1_700_000_000_000)).not.toBe("--");
  });
});
