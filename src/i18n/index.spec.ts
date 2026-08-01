import { afterEach, describe, expect, it } from "vitest";
import { formatDateTime, language, normalizeLanguage, setLanguage, t, useI18n } from "./index";

describe("i18n", () => {
  afterEach(() => {
    setLanguage("zh-CN");
  });

  it("normalizes unsupported languages to simplified Chinese", () => {
    expect(normalizeLanguage("en-US")).toBe("en-US");
    expect(normalizeLanguage("fr-FR")).toBe("zh-CN");
    expect(normalizeLanguage(null)).toBe("zh-CN");
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
