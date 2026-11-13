import { afterEach, expect, it, vi } from "vitest";

vi.mock("../../../../CHANGELOG.md?raw", () => ({ default: "## 99.0.0 - 2099-01-01\n\n- 尚未翻译的版本内容" }));

import { setLanguage } from "../../../i18n";
import { recentChangelogEntries } from "./changelogService";

afterEach(() => setLanguage("zh-CN"));

it("shows a clear English notice for a new release without a translation", () => {
  setLanguage("en-US");
  expect(recentChangelogEntries.value[0].version).toBe("99.0.0");
  expect(recentChangelogEntries.value[0].sections[0].title).toBe("Translation pending");
  expect(JSON.stringify(recentChangelogEntries.value)).not.toMatch(/\p{Script=Han}/u);
  setLanguage("zh-CN");
  expect(recentChangelogEntries.value[0].sections[0].items).toEqual(["尚未翻译的版本内容"]);
});
