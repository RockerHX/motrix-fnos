import { describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";

vi.mock("naive-ui", async () => {
  const { defineComponent, h } = await import("vue");
  const slotStub = (name: string) =>
    defineComponent({
      name,
      setup(_, { slots }) {
        return () => h("div", { "data-test": name }, slots.default?.());
      },
    });

  return {
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
    NCheckbox: defineComponent({
      name: "NCheckboxStub",
      props: {
        checked: {
          type: Boolean,
          default: false,
        },
      },
      emits: ["update:checked"],
      setup(props, { emit }) {
        return () =>
          h("input", {
            type: "checkbox",
            checked: props.checked,
            onChange: (event: Event) => {
              emit("update:checked", (event.target as HTMLInputElement).checked);
            },
          });
      },
    }),
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
    NSpace: slotStub("n-space"),
  };
});

import TaskFileConfirmDialog from "./TaskFileConfirmDialog.vue";
import { mountWithPinia } from "../../../test/mount";
import type { DownloadTask } from "../../../types/tasks";

describe("TaskFileConfirmDialog", () => {
  it("selects all files by default and emits sorted indexes", async () => {
    const { wrapper } = mountDialog();
    await nextTick();

    const checkboxes = wrapper.findAll('input[type="checkbox"]');
    expect(checkboxes).toHaveLength(2);
    expect(checkboxes.every((checkbox) => (checkbox.element as HTMLInputElement).checked)).toBe(true);

    await checkboxes[0].setValue(false);
    await clickButton(wrapper, "开始下载");

    expect(wrapper.emitted("confirm")).toEqual([[[2]]]);
  });

  it("does not confirm when no file is selected", async () => {
    const { wrapper } = mountDialog();
    await nextTick();

    for (const checkbox of wrapper.findAll('input[type="checkbox"]')) {
      await checkbox.setValue(false);
    }
    await clickButton(wrapper, "开始下载");

    expect(wrapper.text()).toContain("请至少选择一个文件");
    expect(wrapper.emitted("confirm")).toBeUndefined();
  });
});

function mountDialog(task: DownloadTask = createTask()) {
  return mountWithPinia(TaskFileConfirmDialog, {
    props: {
      show: true,
      task,
      isLoading: false,
    },
  });
}

function createTask(): DownloadTask {
  return {
    id: 1,
    url: "magnet:?xt=urn:btih:test",
    fileName: "example",
    saveDir: "/downloads",
    category: "默认",
    gid: "gid-1",
    status: "paused",
    totalLength: 0,
    completedLength: 0,
    downloadSpeed: 0,
    errorCode: null,
    errorMessage: null,
    filePath: null,
    confirmationRequired: true,
    files: [
      {
        index: 1,
        path: "/downloads/example/a.mkv",
        name: "a.mkv",
        length: 1024,
        completedLength: 0,
        selected: true,
      },
      {
        index: 2,
        path: "/downloads/example/b.srt",
        name: "b.srt",
        length: 128,
        completedLength: 0,
        selected: true,
      },
    ],
    createdAt: 1,
    updatedAt: 1,
  };
}

async function clickButton(wrapper: ReturnType<typeof mountDialog>["wrapper"], text: string) {
  const button = wrapper.findAll("button").find((item) => item.text() === text);
  if (!button) {
    throw new Error(`button not found: ${text}`);
  }
  await button.trigger("click");
}
