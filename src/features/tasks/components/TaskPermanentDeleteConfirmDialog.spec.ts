import { defineComponent, h } from "vue";
import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";

vi.mock("../../../components/ui/AppConfirmDialog.vue", () => ({
  default: defineComponent({
    props: { show: Boolean },
    emits: ["update:show", "confirm"],
    setup(props, { emit, slots }) {
      return () => props.show ? h("div", [
        h("button", { onClick: () => emit("update:show", false) }, "取消"),
        h("button", { onClick: () => emit("confirm") }, slots["confirm-label"]?.()),
      ]) : null;
    },
  }),
}));

import TaskPermanentDeleteConfirmDialog from "./TaskPermanentDeleteConfirmDialog.vue";
import { defaultConfirmTexts, defaultLabels, defaultState } from "./taskActionTestFixtures";

describe("TaskPermanentDeleteConfirmDialog", () => {
  it("emits only close and permanent delete confirmation", async () => {
    const wrapper = mount(TaskPermanentDeleteConfirmDialog, {
      props: { show: true, state: defaultState, labels: defaultLabels, confirmTexts: defaultConfirmTexts },
    });

    await wrapper.findAll("button")[0].trigger("click");
    await wrapper.findAll("button")[1].trigger("click");
    expect(wrapper.emitted("update:show")).toEqual([[false]]);
    expect(wrapper.emitted("confirm")).toHaveLength(1);
  });
});
