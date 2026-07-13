import { describe, expect, it } from "vitest";
import {
  buildTaskAdvancedOptions,
  createTaskCreateFormState,
  normalizeTaskCategory,
  resetTaskCreateFormState,
} from "./taskCreateFormModel";

describe("taskCreateFormModel", () => {
  it("creates and restores the form defaults", () => {
    const form = createTaskCreateFormState();
    expect(form).toMatchObject({ startMode: "now", category: "默认", connections: 16, downloadLimitKb: 0 });
    form.urls = "https://example.com/file.iso";
    form.category = "电影";

    resetTaskCreateFormState(form);

    expect(form).toEqual(createTaskCreateFormState());
  });

  it("normalizes blank category and proxy values", () => {
    const form = createTaskCreateFormState();
    form.connections = 8;
    form.downloadLimitKb = 512;
    form.proxy = "  ";

    expect(normalizeTaskCategory("  ")).toBe("默认");
    expect(normalizeTaskCategory(" 电影 ")).toBe("电影");
    expect(buildTaskAdvancedOptions(form)).toEqual({ connections: 8, downloadLimitKb: 512, proxy: null });
  });
});
