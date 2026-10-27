import { defineComponent, h } from "vue";
import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";

vi.mock("../../../components/ui/AppConfirmDialog.vue", () => ({
  default: defineComponent({
    props: { show: Boolean },
    emits: ["update:show", "confirm"],
    setup(props, { emit, slots }) {
      return () => props.show ? h("div", [
        slots.extra?.(),
        h("button", { onClick: () => emit("confirm") }, slots["confirm-label"]?.()),
      ]) : null;
    },
  }),
}));

import TaskDeleteConfirmDialog from "./TaskDeleteConfirmDialog.vue";
import { defaultConfirmTexts, defaultLabels, defaultState } from "./taskActionTestFixtures";

describe("TaskDeleteConfirmDialog", () => {
  it("confirms moving to Trash without offering file deletion", async () => {
    const wrapper = mount(TaskDeleteConfirmDialog, {
      props: { show: true, state: defaultState, labels: defaultLabels, confirmTexts: defaultConfirmTexts },
    });
    expect(wrapper.find('input[type="checkbox"]').exists()).toBe(false);
    await wrapper.get("button").trigger("click");
    expect(wrapper.emitted("confirm")).toEqual([[]]);
  });
});
