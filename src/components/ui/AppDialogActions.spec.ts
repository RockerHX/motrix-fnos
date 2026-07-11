import { describe, expect, it, vi } from "vitest";

vi.mock("naive-ui", async () => {
  const { defineComponent, h } = await import("vue");

  return {
    NSpace: defineComponent({
      name: "NSpaceStub",
      setup(_, { slots, attrs }) {
        return () => h("div", { ...attrs, "data-test": "n-space" }, slots.default?.());
      },
    }),
  };
});

import AppDialogActions from "./AppDialogActions.vue";
import { mountWithPinia } from "../../test/mount";

describe("AppDialogActions", () => {
  it("renders action slot inside shared action container", () => {
    const { wrapper } = mountWithPinia(AppDialogActions, {
      slots: {
        default: "操作按钮",
      },
    });

    expect(wrapper.get('[data-test="n-space"]').classes()).toContain("app-dialog-actions");
    expect(wrapper.text()).toContain("操作按钮");
  });
});
