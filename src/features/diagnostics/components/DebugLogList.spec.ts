import { defineComponent, h, nextTick } from "vue";
import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { naiveUiStubs } from "../../../test/mount";
import type { DebugLogEntry } from "../types";

vi.mock("naive-ui", () => ({
  ...naiveUiStubs,
  NEmpty: defineComponent({
    props: { description: String },
    setup(props) {
      return () => h("div", { "data-test": "empty" }, props.description);
    },
  }),
  NTag: defineComponent({
    setup(_, { slots }) {
      return () => h("span", { "data-test": "tag" }, slots.default?.());
    },
  }),
}));

import DebugLogList from "./DebugLogList.vue";
import { setLanguage } from "../../../i18n";

const repeatedLog: DebugLogEntry = {
  id: 1,
  timestampMs: 1_700_000_000_000,
  lastTimestampMs: 1_700_000_060_000,
  level: "warn",
  category: "aria2",
  module: "aria2.rpc",
  message: "retry failed",
  repeatCount: 3,
};

describe("DebugLogList", () => {
  it("updates category labels with the language and preserves unknown categories", async () => {
    const wrapper = mount(DebugLogList, {
      props: { logs: [{ ...repeatedLog, category: "settings" }, { ...repeatedLog, id: 2, category: "constructor" as DebugLogEntry["category"] }], totalCount: 2, active: true },
    });
    expect(wrapper.text()).toContain("设置");
    expect(wrapper.text()).toContain("constructor");
    try {
      setLanguage("en-US");
      await nextTick();
      expect(wrapper.text()).toContain("Settings");
      expect(wrapper.text()).toContain("constructor");
    } finally {
      setLanguage("zh-CN");
      wrapper.unmount();
    }
  });

  it("renders level, category, repeat metadata and message", () => {
    const wrapper = mount(DebugLogList, {
      props: { logs: [repeatedLog], totalCount: 1, active: true },
    });

    expect(wrapper.text()).toContain("WARN");
    expect(wrapper.text()).toContain("Aria2");
    expect(wrapper.text()).toContain("重复 3 次");
    expect(wrapper.text()).toContain("最后");
    expect(wrapper.text()).toContain("aria2.rpc");
    expect(wrapper.text()).toContain("retry failed");
  });

  it("distinguishes empty logs from empty filtered results", async () => {
    const wrapper = mount(DebugLogList, {
      props: { logs: [], totalCount: 0, active: true },
    });
    expect(wrapper.text()).toContain("暂无调试日志");

    await wrapper.setProps({ totalCount: 2 });
    expect(wrapper.text()).toContain("没有匹配当前筛选的日志");
  });
});
