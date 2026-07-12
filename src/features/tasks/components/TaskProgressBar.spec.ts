import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";

vi.mock("naive-ui", async () => {
  const { defineComponent, h } = await import("vue");

  return {
    NProgress: defineComponent({
      name: "NProgressStub",
      props: {
        percentage: {
          type: Number,
          default: 0,
        },
        height: {
          type: Number,
          default: 0,
        },
        color: {
          type: Object,
          default: () => ({}),
        },
        railStyle: {
          type: String,
          default: undefined,
        },
      },
      setup(props) {
        return () =>
          h("div", {
            "data-test": "n-progress",
            "data-percentage": String(props.percentage),
            "data-height": String(props.height),
            "data-color": JSON.stringify(props.color),
            "data-rail-style": props.railStyle,
          });
      },
    }),
  };
});

import TaskProgressBar from "./TaskProgressBar.vue";

describe("TaskProgressBar", () => {
  it("clamps progress percentage to 0-100 before passing it to NProgress", () => {
    const negative = mount(TaskProgressBar, { props: { percentage: -20 } });
    expect(negative.get('[data-test="n-progress"]').attributes("data-percentage")).toBe("0");

    const overflow = mount(TaskProgressBar, { props: { percentage: 140 } });
    expect(overflow.get('[data-test="n-progress"]').attributes("data-percentage")).toBe("100");
  });

  it("uses default tone unless complete tone is requested", () => {
    const wrapper = mount(TaskProgressBar, { props: { percentage: 50 } });
    expect(wrapper.classes()).toContain("task-progress-bar--default");
    expect(wrapper.get('[data-test="n-progress"]').attributes("data-color")).toContain("56%");

    const complete = mount(TaskProgressBar, { props: { percentage: 100, tone: "complete" } });
    expect(complete.classes()).toContain("task-progress-bar--complete");
    expect(complete.get('[data-test="n-progress"]').attributes("data-color")).toContain("72%");
  });

  it("supports empty tone with striped rail and zero progress", () => {
    const wrapper = mount(TaskProgressBar, { props: { percentage: 50, tone: "empty" } });
    const progress = wrapper.get('[data-test="n-progress"]');

    expect(wrapper.classes()).toContain("task-progress-bar--empty");
    expect(progress.attributes("data-percentage")).toBe("0");
    expect(progress.attributes("data-rail-style")).toContain("repeating-linear-gradient");
  });

  it("uses thinner height for card variant", () => {
    const compact = mount(TaskProgressBar, { props: { percentage: 50 } });
    expect(compact.get('[data-test="n-progress"]').attributes("data-height")).toBe("5");

    const card = mount(TaskProgressBar, { props: { percentage: 50, variant: "card" } });
    expect(card.get('[data-test="n-progress"]').attributes("data-height")).toBe("4");
  });
});
