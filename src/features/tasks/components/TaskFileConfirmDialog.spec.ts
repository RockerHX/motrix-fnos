import { describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";

vi.mock("naive-ui", async () => {
  const { defineComponent, h } = await import("vue");
  type DataTableRow = Record<string, unknown>;
  type DataTableColumn = {
    key?: string;
    title?: string;
    type?: string;
    disabled?: (row: DataTableRow) => boolean;
    render?: (row: DataTableRow) => unknown;
  };
  type DataTableRowKey = string | number;
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
    NDataTable: defineComponent({
      name: "NDataTableStub",
      props: {
        columns: {
          type: Array,
          default: () => [],
        },
        data: {
          type: Array,
          default: () => [],
        },
        checkedRowKeys: {
          type: Array,
          default: () => [],
        },
        rowKey: {
          type: Function,
          default: (row: DataTableRow) => row.key,
        },
      },
      emits: ["update:checked-row-keys"],
      setup(props, { emit }) {
        const getColumns = () => props.columns as DataTableColumn[];
        const getRows = () => props.data as DataTableRow[];
        const getCheckedRowKeys = () => props.checkedRowKeys as DataTableRowKey[];
        const getRowKey = (row: DataTableRow, index: number) => (props.rowKey as (row: DataTableRow) => DataTableRowKey)(row) ?? index;

        return () =>
          h("div", { "data-test": "n-data-table" }, [
            ...getRows().map((row, index) => {
              const key = getRowKey(row, index);
              const selectionColumn = getColumns().find((column) => column.type === "selection");
              const checked = getCheckedRowKeys().includes(key);
              const disabled = Boolean(selectionColumn?.disabled?.(row));
              return h("div", { "data-test": "n-data-table-row", "data-row-key": String(key) }, [
                h("input", {
                  type: "checkbox",
                  checked,
                  disabled,
                  onChange: (event: Event) => {
                    if (disabled) {
                      return;
                    }
                    const nextChecked = (event.target as HTMLInputElement).checked;
                    const nextKeys = nextChecked
                      ? [...new Set([...getCheckedRowKeys(), key])]
                      : getCheckedRowKeys().filter((item) => item !== key);
                    emit("update:checked-row-keys", nextKeys);
                  },
                }),
                ...getColumns()
                  .filter((column) => column.type !== "selection")
                  .map((column) =>
                    h(
                      "span",
                      { "data-column-key": column.key },
                      column.render ? String(column.render(row) ?? "") : String(row[column.key ?? ""] ?? ""),
                    ),
                  ),
              ]);
            }),
          ]);
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

    expect(wrapper.find('[data-test="n-data-table"]').exists()).toBe(true);
    const checkboxes = wrapper.findAll('input[type="checkbox"]');
    expect(checkboxes).toHaveLength(2);
    expect(checkboxes.every((checkbox) => (checkbox.element as HTMLInputElement).checked)).toBe(true);

    await checkboxes[0].setValue(false);
    await clickButton(wrapper, "开始下载");

    expect(wrapper.emitted("confirm")).toEqual([[[2]]]);
  });

  it("keeps selected indexes when one file is unchecked", async () => {
    const { wrapper } = mountDialog();
    await nextTick();

    const checkboxes = wrapper.findAll('input[type="checkbox"]');
    await checkboxes[1].setValue(false);
    await clickButton(wrapper, "开始下载");

    expect(wrapper.emitted("confirm")).toEqual([[[1]]]);
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
    useProxy: false,
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
