import { defineComponent, h } from "vue";
import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";

vi.mock("naive-ui", () => ({
  NCheckbox: defineComponent({
    props: { checked: Boolean },
    emits: ["update:checked"],
    setup(props, { emit, slots }) {
      return () => h("label", [
        h("input", {
          type: "checkbox",
          checked: props.checked,
          onChange: (event: Event) => emit("update:checked", (event.target as HTMLInputElement).checked),
        }),
        slots.default?.(),
      ]);
    },
  }),
}));

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
  it("emits the checkbox value and resets it whenever the dialog reopens", async () => {
    const wrapper = mount(TaskDeleteConfirmDialog, {
      props: { show: true, state: defaultState, labels: defaultLabels, confirmTexts: defaultConfirmTexts },
    });

    const checkbox = wrapper.get<HTMLInputElement>('input[type="checkbox"]');
    expect(checkbox.element.checked).toBe(false);
    await checkbox.setValue(true);
    await wrapper.get("button").trigger("click");
    expect(wrapper.emitted("confirm")).toEqual([[true]]);

    await wrapper.setProps({ show: false });
    await wrapper.setProps({ show: true });
    expect(wrapper.get<HTMLInputElement>('input[type="checkbox"]').element.checked).toBe(false);
  });
});
