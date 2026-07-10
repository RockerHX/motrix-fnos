import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import TaskProgressBar from "./TaskProgressBar.vue";

describe("TaskProgressBar", () => {
  it("clamps progress percentage to 0-100", () => {
    const negative = mount(TaskProgressBar, { props: { percentage: -20 } });
    expect(negative.find(".progress-fill").attributes("style")).toContain("scaleX(0)");

    const overflow = mount(TaskProgressBar, { props: { percentage: 140 } });
    expect(overflow.find(".progress-fill").attributes("style")).toContain("scaleX(1)");
  });

  it("uses default tone unless complete tone is requested", () => {
    const wrapper = mount(TaskProgressBar, { props: { percentage: 50 } });
    expect(wrapper.classes()).toContain("task-progress-bar--default");

    const complete = mount(TaskProgressBar, { props: { percentage: 100, tone: "complete" } });
    expect(complete.classes()).toContain("task-progress-bar--complete");
  });
});
