import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Topbar from "./Topbar.vue";

describe("Topbar", () => {
  it("renders desktop toolbar actions in order and emits events", async () => {
    const wrapper = mount(Topbar, {
      props: {
        activeCategory: "downloading",
      },
    });

    const buttons = wrapper.findAll(".desktop-actions > button");
    expect(buttons.map((button) => button.attributes("aria-label"))).toEqual([
      "新建任务",
      "刷新",
      "暂停当前可见任务",
      "继续当前可见任务",
      "删除当前可见任务",
    ]);

    for (const button of buttons) {
      await button.trigger("click");
    }

    expect(wrapper.emitted("create")).toHaveLength(1);
    expect(wrapper.emitted("refresh")).toHaveLength(1);
    expect(wrapper.emitted("pauseVisible")).toHaveLength(1);
    expect(wrapper.emitted("resumeVisible")).toHaveLength(1);
    expect(wrapper.emitted("deleteVisible")).toHaveLength(1);
  });

  it("keeps mobile auxiliary actions", () => {
    const wrapper = mount(Topbar, {
      props: {
        activeCategory: "downloading",
      },
    });

    expect(wrapper.findAll(".mobile-actions > button").map((button) => button.attributes("aria-label"))).toEqual([
      "设置",
      "帮助",
      "关于",
      "诊断",
    ]);
  });
});
