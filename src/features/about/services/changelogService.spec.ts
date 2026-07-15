import { describe, expect, it } from "vitest";
import changelogMarkdown from "../../../../CHANGELOG.md?raw";
import { parseChangelog } from "./changelogService";

describe("parseChangelog", () => {
  it("parses multiple versions and sections in source order", () => {
    const entries = parseChangelog(`# Changelog

## 1.2.0 - 2026-07-13

### 新增

- 增加批量任务
- 增加移动布局

### 修复

- 修复下载状态

## 1.1.0 - 2026-07-01

### 改进

- 优化日志
`);

    expect(entries).toEqual([
      {
        version: "1.2.0",
        date: "2026-07-13",
        sections: [
          { title: "新增", items: ["增加批量任务", "增加移动布局"] },
          { title: "修复", items: ["修复下载状态"] },
        ],
      },
      {
        version: "1.1.0",
        date: "2026-07-01",
        sections: [{ title: "改进", items: ["优化日志"] }],
      },
    ]);
  });

  it("accepts version headings without dates", () => {
    expect(parseChangelog("## 2.0.0\n\n### 新增\n\n- 新版本")).toEqual([
      {
        version: "2.0.0",
        date: "",
        sections: [{ title: "新增", items: ["新版本"] }],
      },
    ]);
  });

  it("keeps legacy bullets before the first named section as changes", () => {
    expect(parseChangelog(`## 2.0.0 - 2026-07-15

- 未分类的历史变更

### 修复

- 已分类的修复
`)).toEqual([
      {
        version: "2.0.0",
        date: "2026-07-15",
        sections: [
          { title: "变更", items: ["未分类的历史变更"] },
          { title: "修复", items: ["已分类的修复"] },
        ],
      },
    ]);
  });

  it("reads legacy entries from the real changelog without duplicating their content", () => {
    const entry = parseChangelog(changelogMarkdown).find(({ version }) => version === "1.7.2");

    expect(entry).toBeDefined();
    expect(entry?.sections[0]?.title).toBe("变更");
    expect(entry?.sections[0]?.items.length).toBeGreaterThan(0);
    expect(entry?.sections.some(({ title, items }) => title === "文档" && items.length > 0)).toBe(true);
    expect(entry?.sections.some(({ title, items }) => title === "改进" && items.length > 0)).toBe(true);
  });

  it("ignores bullets outside sections and unrelated markdown", () => {
    expect(parseChangelog("# Title\n\n- orphan\n\ntext\n\n### 修复\n\n- still orphan")).toEqual([]);
  });

  it("drops versions whose sections contain no items", () => {
    expect(parseChangelog("## 1.0.0 - 2026-01-01\n\n### 新增\n\n说明文字")).toEqual([]);
  });

  it("keeps non-empty sections when the same version also has empty sections", () => {
    expect(parseChangelog("## 1.0.0\n\n### 新增\n\n### 修复\n\n- 已修复")).toEqual([
      {
        version: "1.0.0",
        date: "",
        sections: [
          { title: "新增", items: [] },
          { title: "修复", items: ["已修复"] },
        ],
      },
    ]);
  });
});
