import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import AppMetricCard from "./AppMetricCard.vue";

describe("AppMetricCard", () => {
  it("renders label value detail and note from props", () => {
    const wrapper = mount(AppMetricCard, {
      props: {
        label: "进程",
        value: "运行中",
        detail: "Aria2 已启动",
        note: "PID：123",
        tone: "success",
      },
    });

    expect(wrapper.classes()).toContain("app-metric-card--success");
    expect(wrapper.get(".app-metric-label").text()).toBe("进程");
    expect(wrapper.get(".app-metric-value").text()).toBe("运行中");
    expect(wrapper.get(".app-metric-detail").text()).toBe("Aria2 已启动");
    expect(wrapper.get(".app-metric-note").text()).toBe("PID：123");
  });

  it("allows slots to override metric content", () => {
    const wrapper = mount(AppMetricCard, {
      props: {
        label: "label",
        value: "value",
        detail: "detail",
        note: "note",
        tone: "warning",
      },
      slots: {
        label: "自定义标签",
        value: "自定义值",
        detail: "自定义详情",
        note: "自定义备注",
      },
    });

    expect(wrapper.classes()).toContain("app-metric-card--warning");
    expect(wrapper.text()).toContain("自定义标签");
    expect(wrapper.text()).toContain("自定义值");
    expect(wrapper.text()).toContain("自定义详情");
    expect(wrapper.text()).toContain("自定义备注");
  });
});
