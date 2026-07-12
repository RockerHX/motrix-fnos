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

  it("renders desktop auxiliary actions and emits their events", async () => {
    const wrapper = mount(SidebarNav, {
      props: {
        appInfo: null,
        activeCategory: "downloading",
      },
    });

    const actions = wrapper.findAll(".sidebar-footer button");
    expect(actions.map((action) => action.attributes("aria-label"))).toEqual(["设置", "帮助", "关于", "诊断"]);
    expect(actions.map((action) => action.get("svg").attributes("data-icon-name"))).toEqual([
      "settings",
      "help",
      "about",
      "diagnostics",
    ]);

    await actions[0].trigger("click");
    await actions[1].trigger("click");
    await actions[2].trigger("click");
    await actions[3].trigger("click");

    expect(wrapper.emitted("openSettings")).toHaveLength(1);
    expect(wrapper.emitted("openHelp")).toHaveLength(1);
    expect(wrapper.emitted("openAbout")).toHaveLength(1);
    expect(wrapper.emitted("openDiagnostics")).toHaveLength(1);
  });
});
