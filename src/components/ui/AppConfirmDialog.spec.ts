import { describe, expect, it, vi } from "vitest";

vi.mock("naive-ui", async () => {
  const { defineComponent, h } = await import("vue");

  return {
    NModal: defineComponent({
      name: "NModalStub",
      props: { show: { type: Boolean, default: false } },
      emits: ["update:show"],
      setup(props, { slots }) {
        return () => (props.show ? h("div", { "data-test": "n-modal" }, slots.default?.()) : null);
      },
    }),
    NCard: defineComponent({
      name: "NCardStub",
      setup(_, { slots }) {
        return () => h("div", { "data-test": "n-card" }, [slots.header?.(), slots.default?.(), slots.footer?.()]);
      },
    }),
    NButton: defineComponent({
      name: "NButtonStub",
      props: {
        disabled: { type: Boolean, default: false },
        loading: { type: Boolean, default: false },
      },
      emits: ["click"],
      setup(props, { emit, slots, attrs }) {
        return () =>
          h(
            "button",
            {
              ...attrs,
              disabled: props.disabled,
              "data-loading": props.loading ? "true" : "false",
              onClick: (event: MouseEvent) => {
                if (!props.disabled) emit("click", event);
              },
            },
            slots.default?.(),
          );
      },
    }),
    NSpace: defineComponent({
      name: "NSpaceStub",
      setup(_, { slots }) {
        return () => h("div", { "data-test": "n-space" }, slots.default?.());
      },
    }),
  };
});

import AppConfirmDialog from "./AppConfirmDialog.vue";
import { flushPromises, mountWithPinia } from "../../test/mount";

describe("AppConfirmDialog", () => {
  it("renders title, text, extra slot and confirm label", () => {
    const { wrapper } = mountWithPinia(AppConfirmDialog, {
      props: {
        show: true,
        title: "确认操作",
        confirmText: "确定继续吗？",
      },
      slots: {
        extra: "额外选项",
        "confirm-label": "继续",
      },
    });

    expect(wrapper.text()).toContain("确认操作");
    expect(wrapper.text()).toContain("确定继续吗？");
    expect(wrapper.text()).toContain("额外选项");
    expect(wrapper.text()).toContain("继续");
  });

  it("emits cancel and confirm events", async () => {
    const { wrapper } = mountWithPinia(AppConfirmDialog, {
      props: {
        show: true,
        title: "确认操作",
      },
    });

    await clickButton(wrapper, "取消");
    await clickButton(wrapper, "确认");

    expect(wrapper.emitted("cancel")).toHaveLength(1);
    expect(wrapper.emitted("update:show")).toEqual([[false]]);
    expect(wrapper.emitted("confirm")).toHaveLength(1);
  });

  it("disables actions while loading", async () => {
    const { wrapper } = mountWithPinia(AppConfirmDialog, {
      props: {
        show: true,
        title: "确认操作",
        loading: true,
      },
    });

    for (const button of wrapper.findAll("button")) {
      await button.trigger("click");
    }
    await flushPromises();

    expect(wrapper.emitted("cancel")).toBeUndefined();
    expect(wrapper.emitted("confirm")).toBeUndefined();
  });
});

async function clickButton(wrapper: ReturnType<typeof mountWithPinia>["wrapper"], text: string) {
  const button = wrapper.findAll("button").find((item) => item.text() === text);
  expect(button, `button ${text} should exist`).toBeTruthy();
  await button!.trigger("click");
  await flushPromises();
}
