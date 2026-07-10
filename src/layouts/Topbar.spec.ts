import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Topbar from "./Topbar.vue";
import { setLanguage } from "../i18n";

describe("Topbar", () => {
  afterEach(() => {
    setLanguage("zh-CN");
  });

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

  it("keeps English aria labels and titles functional", async () => {
    setLanguage("en-US");
    const wrapper = mount(Topbar, {
      props: {
        activeCategory: "downloading",
      },
    });

    const refreshButton = wrapper.get('button[aria-label="Refresh"]');
    expect(refreshButton.attributes("title")).toBe("Refresh");

    await refreshButton.trigger("click");
    expect(wrapper.emitted("refresh")).toHaveLength(1);
  });

  it("does not emit disabled desktop actions and keeps disabled title", async () => {
    const wrapper = mount(Topbar, {
      props: {
        activeCategory: "downloading",
        actionStates: {
          pauseVisible: { disabled: true, title: "当前没有可暂停的任务" },
        },
      },
    });

    const pauseButton = wrapper.get('button[aria-label="暂停当前可见任务"]');
    expect(pauseButton.attributes("disabled")).toBeDefined();
    expect(pauseButton.attributes("title")).toBe("当前没有可暂停的任务");

    await pauseButton.trigger("click");
    expect(wrapper.emitted("pauseVisible")).toBeUndefined();
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
