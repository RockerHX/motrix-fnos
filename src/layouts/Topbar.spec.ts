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
    expect(buttons.map((button) => button.get("svg").attributes("data-icon-name"))).toEqual([
      "plus",
      "refresh",
      "pause",
      "play",
      "trash",
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

  it("replaces visible-delete with empty-trash in the Trash category", async () => {
    const wrapper = mount(Topbar, {
      props: {
        activeCategory: "trash",
      },
    });

    expect(wrapper.find('button[aria-label="删除当前可见任务"]').exists()).toBe(false);
    const clearButton = wrapper.get('.desktop-actions > button[aria-label="清空回收站"]');
    expect(clearButton.get("svg").attributes("data-icon-name")).toBe("trash");
    expect(wrapper.find('.mobile-actions > button[aria-label="清空回收站"]').exists()).toBe(true);

    await clearButton.trigger("click");
    expect(wrapper.emitted("clearTrash")).toHaveLength(1);
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
      "退出登录",
    ]);
    expect(wrapper.find(".topbar-more").exists()).toBe(false);
  });

  it("disables mobile logout and does not emit while logout is loading", async () => {
    const wrapper = mount(Topbar, {
      props: {
        activeCategory: "downloading",
        logoutLoading: true,
      },
    });

    const logoutButton = wrapper.get('.mobile-actions > button[aria-label="退出登录"]');
    expect(logoutButton.attributes("disabled")).toBeDefined();

    await logoutButton.trigger("click");
    expect(wrapper.emitted("logout")).toBeUndefined();
  });
});
