import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import SidebarNav from "./SidebarNav.vue";

describe("SidebarNav", () => {
  it("renders All first and omits the stopped category", () => {
    const wrapper = mount(SidebarNav, {
      props: {
        appInfo: null,
        activeCategory: "all",
      },
    });

    const labels = wrapper.findAll(".category-list button").map((item) => item.attributes("aria-label"));
    expect(labels).toEqual(["全部", "下载中", "已完成", "回收站", "扩展"]);
  });

  it("marks the active category and emits one category selection", async () => {
    const wrapper = mount(SidebarNav, {
      props: {
        appInfo: null,
        activeCategory: "downloading",
      },
    });

    const activeButton = wrapper.get('.category-list button[aria-label="下载中"]');
    expect(activeButton.attributes("aria-current")).toBe("page");

    await wrapper.get('.category-list button[aria-label="已完成"]').trigger("click");

    expect(wrapper.emitted("selectCategory")).toEqual([["completed"]]);
  });

  it("renders desktop auxiliary actions and emits their events", async () => {
    const wrapper = mount(SidebarNav, {
      props: {
        appInfo: null,
        activeCategory: "downloading",
      },
    });

    const actions = wrapper.findAll(".sidebar-footer button");
    expect(actions.map((action) => action.attributes("aria-label"))).toEqual(["设置", "帮助", "关于", "诊断", "退出登录"]);
    expect(actions.map((action) => action.get("svg").attributes("data-icon-name"))).toEqual([
      "settings",
      "help",
      "about",
      "diagnostics",
      "logout",
    ]);

    await actions[0].trigger("click");
    await actions[1].trigger("click");
    await actions[2].trigger("click");
    await actions[3].trigger("click");
    await actions[4].trigger("click");

    expect(wrapper.emitted("openSettings")).toHaveLength(1);
    expect(wrapper.emitted("openHelp")).toHaveLength(1);
    expect(wrapper.emitted("openAbout")).toHaveLength(1);
    expect(wrapper.emitted("openDiagnostics")).toHaveLength(1);
    expect(wrapper.emitted("logout")).toHaveLength(1);
  });

  it("disables logout and does not emit while logout is loading", async () => {
    const wrapper = mount(SidebarNav, {
      props: {
        appInfo: null,
        activeCategory: "all",
        logoutLoading: true,
      },
    });

    const logoutButton = wrapper.get('.sidebar-footer button[aria-label="退出登录"]');
    expect(logoutButton.attributes("disabled")).toBeDefined();

    await logoutButton.trigger("click");
    expect(wrapper.emitted("logout")).toBeUndefined();
  });
});
