import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import TaskProgressBar from "./TaskProgressBar.vue";

describe("TaskProgressBar", () => {
  it("normalizes finite percentages and rejects non-finite values", () => {
    const cases = [
      { percentage: -20, expected: "0", scale: "0" },
      { percentage: 140, expected: "100", scale: "1" },
      { percentage: Number.NaN, expected: "0", scale: "0" },
      { percentage: Number.POSITIVE_INFINITY, expected: "0", scale: "0" },
    ];

    for (const { percentage, expected, scale } of cases) {
      const wrapper = mount(TaskProgressBar, { props: { percentage } });

      expect(wrapper.attributes("aria-valuenow")).toBe(expected);
      expect(wrapper.element.style.getPropertyValue("--task-progress-scale")).toBe(scale);
    }
  });

  it("exposes the normalized scale through the public progressbar root", () => {
    const wrapper = mount(TaskProgressBar, { props: { percentage: 50 } });

    expect(wrapper.attributes("role")).toBe("progressbar");
    expect(wrapper.attributes("aria-valuemin")).toBe("0");
    expect(wrapper.attributes("aria-valuemax")).toBe("100");
    expect(wrapper.attributes("aria-valuenow")).toBe("50");
    expect(wrapper.element.style.getPropertyValue("--task-progress-scale")).toBe("0.5");
  });

  it("forces empty tone to zero and preserves its rail class", () => {
    const wrapper = mount(TaskProgressBar, {
      props: { percentage: 50, tone: "empty" },
    });

    expect(wrapper.classes()).toContain("task-progress-bar--empty");
    expect(wrapper.attributes("aria-valuenow")).toBe("0");
    expect(wrapper.element.style.getPropertyValue("--task-progress-scale")).toBe("0");
    expect(wrapper.find(".task-progress-bar__fill").exists()).toBe(true);
  });

  it("keeps default and complete tone classes with a shared fill element", () => {
    const defaultTone = mount(TaskProgressBar, { props: { percentage: 50 } });
    const completeTone = mount(TaskProgressBar, {
      props: { percentage: 100, tone: "complete" },
    });

    expect(defaultTone.classes()).toContain("task-progress-bar--default");
    expect(defaultTone.find(".task-progress-bar__fill").exists()).toBe(true);
    expect(completeTone.classes()).toContain("task-progress-bar--complete");
    expect(completeTone.find(".task-progress-bar__fill").exists()).toBe(true);
  });

  it("preserves compact and card variant classes", () => {
    const compact = mount(TaskProgressBar, { props: { percentage: 50 } });
    const card = mount(TaskProgressBar, {
      props: { percentage: 50, variant: "card" },
    });

    expect(compact.classes()).toContain("task-progress-bar--compact");
    expect(card.classes()).toContain("task-progress-bar--card");
  });

  it("does not render the removed Naive UI progress component", () => {
    const wrapper = mount(TaskProgressBar, { props: { percentage: 50 } });

    expect(wrapper.find('[data-test="n-progress"]').exists()).toBe(false);
  });
});
