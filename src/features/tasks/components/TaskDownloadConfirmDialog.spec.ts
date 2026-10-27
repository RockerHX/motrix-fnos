import { defineComponent, h } from "vue";
import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";

vi.mock("../../../components/ui/AppConfirmDialog.vue", () => ({
  default: defineComponent({
    props: { show: Boolean, title: String, loading: Boolean, disabled: Boolean, maskClosable: Boolean },
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

import TaskDownloadConfirmDialog from "./TaskDownloadConfirmDialog.vue";
import { defaultConfirmTexts, defaultLabels, defaultState } from "./taskActionTestFixtures";

describe("TaskDownloadConfirmDialog", () => {
  it.each(["redownload", "restore"] as const)("renders %s text and preserves proxy confirmation and operating state", async (operation) => {
    const wrapper = mount(TaskDownloadConfirmDialog, {
      props: {
        show: true,
        title: defaultConfirmTexts[`${operation}Title`],
        text: defaultConfirmTexts[`${operation}ConfirmText`],
        confirmLabel: defaultLabels[operation],
        state: { ...defaultState, isOperating: true },
        useProxy: true,
      },
    });
    expect(wrapper.text()).toContain(defaultConfirmTexts[`${operation}ConfirmText`]);
    const dialog = wrapper.get('[data-test="confirm-dialog"]');
    expect(dialog.attributes("data-loading")).toBe("true");
    expect(dialog.attributes("data-mask-closable")).toBe("false");
    await wrapper.findAll("button").find((button) => button.text() === "取消")!.trigger("click");
    await wrapper.findAll("button").find((button) => button.text() === defaultLabels[operation])!.trigger("click");
    expect(wrapper.emitted("update:show")).toEqual([[false]]);
    expect(wrapper.emitted("confirm")).toEqual([[true]]);
  });
});
