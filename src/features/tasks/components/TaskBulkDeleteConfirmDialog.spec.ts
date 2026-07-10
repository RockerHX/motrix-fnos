import { describe, expect, it, vi } from "vitest";

vi.mock("naive-ui", async () => {
  const { defineComponent, h } = await import("vue");

  return {
    NModal: defineComponent({
      name: "NModalStub",
      props: {
        show: {
          type: Boolean,
          default: false,
        },
      },
      setup(props, { slots }) {
        return () => (props.show ? h("div", { "data-test": "n-modal" }, slots.default?.()) : null);
      },
    }),
    NCard: defineComponent({
      name: "NCardStub",
      setup(_, { slots }) {
        return () =>
          h("div", { "data-test": "n-card" }, [
            ...(slots.default?.() ?? []),
            ...(slots.footer?.() ?? []),
          ]);
      },
    }),
    NButton: defineComponent({
      name: "NButtonStub",
      props: {
        disabled: {
          type: Boolean,
          default: false,
        },
      },
      emits: ["click"],
      setup(props, { emit, slots, attrs }) {
        return () =>
          h(
            "button",
            {
              ...attrs,
              disabled: props.disabled,
              onClick: (event: MouseEvent) => {
                if (!props.disabled) {
                  emit("click", event);
                }
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

import TaskBulkDeleteConfirmDialog from "./TaskBulkDeleteConfirmDialog.vue";
import { flushPromises, mountWithPinia } from "../../../test/mount";

describe("TaskBulkDeleteConfirmDialog", () => {
  it("shows delete count when opened", () => {
    const { wrapper } = mountWithPinia(TaskBulkDeleteConfirmDialog, {
      props: {
        show: true,
        taskCount: 3,
      },
    });

    expect(wrapper.text()).toContain("3");
    expect(wrapper.text()).toContain("不会删除本地文件");
  });

  it("closes without confirming when canceled", async () => {
    const { wrapper } = mountWithPinia(TaskBulkDeleteConfirmDialog, {
      props: {
        show: true,
        taskCount: 2,
      },
    });

    await clickButton(wrapper, "取消");

    expect(wrapper.emitted("update:show")).toEqual([[false]]);
    expect(wrapper.emitted("confirm")).toBeUndefined();
  });

  it("emits confirm when confirmed", async () => {
    const { wrapper } = mountWithPinia(TaskBulkDeleteConfirmDialog, {
      props: {
        show: true,
        taskCount: 2,
      },
    });

    await clickButton(wrapper, "删除");

    expect(wrapper.emitted("confirm")).toHaveLength(1);
  });
});

async function clickButton(wrapper: ReturnType<typeof mountWithPinia>["wrapper"], text: string) {
  const button = wrapper.findAll("button").find((item) => item.text() === text);
  expect(button, `button ${text} should exist`).toBeTruthy();
  await button!.trigger("click");
  await flushPromises();
}
