import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import AppMetricGrid from "./AppMetricGrid.vue";

describe("AppMetricGrid", () => {
  it("renders metric cards from items", () => {
    const wrapper = mount(AppMetricGrid, {
      props: {
        items: [
          { label: "总数", value: 3, detail: "全部日志" },
          { label: "错误", value: 1, note: "需要关注", tone: "error" },
        ],
      },
    });

    const cards = wrapper.findAll(".app-metric-card");
    expect(cards).toHaveLength(2);
    expect(wrapper.text()).toContain("总数");
    expect(wrapper.text()).toContain("全部日志");
    expect(wrapper.text()).toContain("错误");
    expect(wrapper.text()).toContain("需要关注");
    expect(cards[1].classes()).toContain("app-metric-card--error");
  });

  it("sets desktop and mobile column css variables", () => {
    const wrapper = mount(AppMetricGrid, {
      props: {
        desktopColumns: 5,
        mobileColumns: 2,
        items: [{ label: "总数", value: 3 }],
      },
    });

    expect(wrapper.attributes("style")).toContain("--app-metric-grid-desktop-columns: 5");
    expect(wrapper.attributes("style")).toContain("--app-metric-grid-mobile-columns: 2");
  });
});
