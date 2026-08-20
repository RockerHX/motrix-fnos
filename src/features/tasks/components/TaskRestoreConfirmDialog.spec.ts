import { defineComponent, h } from "vue";
import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";

vi.mock("../../../components/ui/AppConfirmDialog.vue", () => ({
  default: defineComponent({
    props: { show: Boolean, loading: Boolean, disabled: Boolean, maskClosable: Boolean },
    emits: ["update:show", "confirm"],
    setup(props, { emit, slots }) {
      return () =>
        props.show
          ? h(
              "div",
              {
                "data-test": "confirm-dialog",
                "data-loading": String(props.loading),
                "data-disabled": String(props.disabled),
                "data-mask-closable": String(props.maskClosable),
              },
              [
                slots.default?.(),
                h("button", { onClick: () => emit("update:show", false) }, "取消"),
                h("button", { disabled: props.disabled, onClick: () => emit("confirm") }, slots["confirm-label"]?.()),
              ],
            )
          : null;
    },
  }),
}));

import TaskRestoreConfirmDialog from "./TaskRestoreConfirmDialog.vue";
import { defaultConfirmTexts, defaultLabels, defaultState } from "./taskActionTestFixtures";

describe("TaskRestoreConfirmDialog", () => {
  it("inherits proxy use and emits it with the restore confirmation", async () => {
    const wrapper = mount(TaskRestoreConfirmDialog, {
      props: {
        show: true,
        state: defaultState,
        labels: defaultLabels,
        confirmTexts: defaultConfirmTexts,
        useProxy: true,
      },
    });

    expect(wrapper.text()).toContain("确认恢复");
    await wrapper.findAll("button").find((button) => button.text() === "恢复")!.trigger("click");
    expect(wrapper.emitted("confirm")).toEqual([[true]]);
  });
});
