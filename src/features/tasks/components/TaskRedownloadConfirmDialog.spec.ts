import { defineComponent, h } from "vue";
import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";

vi.mock("../../../components/ui/AppConfirmDialog.vue", () => ({
  default: defineComponent({
    props: { show: Boolean, loading: Boolean, disabled: Boolean, maskClosable: Boolean },
    emits: ["update:show", "confirm"],
    setup(props, { emit, slots }) {
      return () => props.show ? h("div", {
        "data-test": "confirm-dialog",
        "data-loading": String(props.loading),
        "data-disabled": String(props.disabled),
        "data-mask-closable": String(props.maskClosable),
      }, [
        h("button", { onClick: () => emit("update:show", false) }, "取消"),
        h("button", { disabled: props.disabled, onClick: () => emit("confirm") }, slots["confirm-label"]?.()),
      ]) : null;
    },
  }),
}));

import TaskRedownloadConfirmDialog from "./TaskRedownloadConfirmDialog.vue";
import { defaultConfirmTexts, defaultLabels, defaultState } from "./taskActionTestFixtures";

describe("TaskRedownloadConfirmDialog", () => {
  it("forwards operating state and emits cancel and confirm", async () => {
    const wrapper = mount(TaskRedownloadConfirmDialog, {
      props: {
        show: true,
        state: { ...defaultState, isOperating: true },
        labels: defaultLabels,
        confirmTexts: defaultConfirmTexts,
      },
    });
    const dialog = wrapper.get('[data-test="confirm-dialog"]');
    expect(dialog.attributes("data-loading")).toBe("true");
    expect(dialog.attributes("data-mask-closable")).toBe("false");

    await wrapper.findAll("button")[0].trigger("click");
    await wrapper.findAll("button")[1].trigger("click");
    expect(wrapper.emitted("update:show")).toEqual([[false]]);
    expect(wrapper.emitted("confirm")).toHaveLength(1);
  });
});
