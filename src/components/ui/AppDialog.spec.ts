import { describe, expect, it, vi } from "vitest";

vi.mock("naive-ui", async () => {
  const { defineComponent, h } = await import("vue");

  return {
    NModal: defineComponent({
      name: "NModalStub",
      props: {
        show: { type: Boolean, default: false },
        maskClosable: { type: Boolean, default: true },
      },
      emits: ["update:show"],
      setup(props, { emit, slots }) {
        return () =>
          props.show
            ? h(
                "div",
                {
                  "data-test": "n-modal",
                  onClick: () => props.maskClosable && emit("update:show", false),
                },
                slots.default?.(),
              )
            : null;
      },
    }),
    NCard: defineComponent({
      name: "NCardStub",
      props: {
        contentClass: { type: String, default: "" },
        contentStyle: { type: [String, Object], default: undefined },
      },
      setup(props, { slots, attrs }) {
        return () =>
          h(
            "section",
            {
              ...attrs,
              "data-test": "n-card",
              "data-content-class": props.contentClass,
              "data-content-style":
                typeof props.contentStyle === "string" ? props.contentStyle : JSON.stringify(props.contentStyle),
            },
            [
              ...(slots.header?.() ?? []),
              ...(slots["header-extra"]?.() ?? []),
              ...(slots.default?.() ?? []),
              ...(slots.footer?.() ?? []),
            ],
          );
      },
    }),
    NButton: defineComponent({
      name: "NButtonStub",
      props: { disabled: { type: Boolean, default: false } },
      emits: ["click"],
      setup(props, { emit, slots, attrs }) {
        return () =>
          h(
            "button",
            {
              ...attrs,
              disabled: props.disabled,
              onClick: (event: MouseEvent) => {
                event.stopPropagation();
                if (!props.disabled) emit("click", event);
              },
            },
            slots.default?.(),
          );
      },
    }),
  };
});

import AppDialog from "./AppDialog.vue";
import { flushPromises, mountWithPinia } from "../../test/mount";

describe("AppDialog", () => {
  it("renders title, eyebrow, default slot and footer", () => {
    const { wrapper } = mountWithPinia(AppDialog, {
      props: {
        show: true,
        title: "弹窗标题",
        eyebrow: "Dialog",
        width: "640px",
      },
      slots: {
        default: "正文内容",
        footer: "底部内容",
      },
    });

    expect(wrapper.text()).toContain("Dialog");
    expect(wrapper.text()).toContain("弹窗标题");
    expect(wrapper.text()).toContain("正文内容");
    expect(wrapper.text()).toContain("底部内容");
    expect(wrapper.get('[data-test="n-card"]').attributes("style")).toContain("--app-dialog-width: 640px");
  });

  it("forwards content class and style to the card body", () => {
    const { wrapper } = mountWithPinia(AppDialog, {
      props: {
        show: true,
        contentClass: "dialog-content",
        contentStyle: "display: flex;",
      },
    });

    const card = wrapper.get('[data-test="n-card"]');
    expect(card.attributes("data-content-class")).toBe("dialog-content");
    expect(card.attributes("data-content-style")).toBe("display: flex;");
  });

  it("emits close events from close button", async () => {
    const { wrapper } = mountWithPinia(AppDialog, {
      props: {
        show: true,
        title: "弹窗标题",
      },
    });

    await wrapper.get('button[aria-label="关闭"]').trigger("click");
    await flushPromises();

    expect(wrapper.emitted("update:show")).toEqual([[false]]);
    expect(wrapper.emitted("close")).toHaveLength(1);
  });

  it("emits update when modal requests hide from mask", async () => {
    const { wrapper } = mountWithPinia(AppDialog, {
      props: {
        show: true,
        showClose: false,
      },
    });

    await wrapper.get('[data-test="n-modal"]').trigger("click");
    await flushPromises();

    expect(wrapper.emitted("update:show")).toEqual([[false]]);
  });

  it("does not close when maskClosable is false", async () => {
    const { wrapper } = mountWithPinia(AppDialog, {
      props: {
        show: true,
        showClose: false,
        maskClosable: false,
      },
    });

    await wrapper.get('[data-test="n-modal"]').trigger("click");
    await flushPromises();

    expect(wrapper.emitted("update:show")).toBeUndefined();
  });

  it("does not close when closeDisabled is true", async () => {
    const { wrapper } = mountWithPinia(AppDialog, {
      props: {
        show: true,
        title: "弹窗标题",
        closeDisabled: true,
      },
    });

    await wrapper.get('button[aria-label="关闭"]').trigger("click");
    await wrapper.get('[data-test="n-modal"]').trigger("click");
    await flushPromises();

    expect(wrapper.emitted("update:show")).toBeUndefined();
  });
});
