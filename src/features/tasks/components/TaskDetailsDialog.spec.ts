import { defineComponent, h } from "vue";
import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { naiveUiStubs } from "../../../test/mount";

vi.mock("naive-ui", () => ({
  ...naiveUiStubs,
  useMessage: () => ({ success: vi.fn(), warning: vi.fn(), error: vi.fn() }),
  NCollapse: defineComponent({
    name: "NCollapseStub",
    props: { defaultExpandedNames: { type: Array, default: () => [] } },
    setup(props, { slots }) {
      return () =>
        h(
          "div",
          {
            "data-test": "n-collapse",
            "data-default-expanded-names": JSON.stringify(props.defaultExpandedNames),
          },
          slots.default?.(),
        );
    },
  }),
  NModal: defineComponent({
    name: "NModalStub",
    props: { show: { type: Boolean, default: false } },
    emits: ["update:show"],
    setup(props, { emit, slots }) {
      return () =>
        props.show
          ? h(
              "div",
              {
                "data-test": "n-modal",
                onClick: (event: MouseEvent) => {
                  if (event.target === event.currentTarget) {
                    emit("update:show", false);
                  }
                },
              },
              slots.default?.(),
            )
          : null;
    },
  }),
  NCard: defineComponent({
    setup(_, { slots }) {
      return () => h("div", [slots.header?.(), slots["header-extra"]?.(), slots.default?.(), slots.footer?.()]);
    },
  }),
  NDescriptionsItem: defineComponent({
    props: { label: String },
    setup(props, { slots }) {
      return () => h("div", { "data-test": "detail-item" }, [props.label, slots.default?.()]);
    },
  }),
}));

vi.mock("../../../app/utils/clipboard", () => ({
  copyTextToClipboard: vi.fn(async () => ({ copied: true, method: "clipboard" })),
}));

import TaskDetailsDialog from "./TaskDetailsDialog.vue";
import { copyTextToClipboard } from "../../../app/utils/clipboard";
import type { DownloadTask } from "../../../types/tasks";

describe("TaskDetailsDialog", () => {
  it("does not render content while hidden", () => {
    const wrapper = mount(TaskDetailsDialog, {
      props: {
        show: false,
        closeLabel: "关闭",
        details: { title: "任务详情", items: [] },
        task: createTask(),
        isOperating: false,
        isActionDisabled: false,
      },
    });

    expect(wrapper.find('[data-test="n-modal"]').exists()).toBe(false);
  });

  it("renders ordered details and emits close", async () => {
    const wrapper = mount(TaskDetailsDialog, {
      props: {
        show: true,
        closeLabel: "关闭",
        details: {
          title: "任务详情",
          items: [
            { label: "任务名称", value: "file.iso" },
            { label: "状态", value: "下载中" },
          ],
        },
        task: createTask(),
        isOperating: false,
        isActionDisabled: false,
      },
    });

    expect(wrapper.findAll('[data-test="detail-item"]').map((item) => item.text())).toEqual([
      "任务名称file.iso",
      "状态下载中",
    ]);
    await wrapper.findAll("button").find((button) => button.text() === "关闭")!.trigger("click");
    expect(wrapper.emitted("update:show")).toEqual([[false]]);
  });

  it("forwards a modal hide request", async () => {
    const wrapper = mount(TaskDetailsDialog, {
      props: {
        show: true,
        closeLabel: "关闭",
        details: { title: "任务详情", items: [] },
        task: createTask(),
        isOperating: false,
        isActionDisabled: false,
      },
    });

    await wrapper.get('[data-test="n-modal"]').trigger("click");

    expect(wrapper.emitted("update:show")).toEqual([[false]]);
  });

  it("shows host file actions and technical path details", async () => {
    const wrapper = mount(TaskDetailsDialog, {
      props: {
        show: true,
        closeLabel: "关闭",
        details: {
          title: "任务详情",
          items: [{ label: "保存路径", value: "存储空间1/下载" }],
          technicalItems: [{ label: "保存路径", value: "/vol1/downloads" }],
        },
        task: createTask({ status: "complete" }),
        isOperating: false,
        isActionDisabled: false,
        fileActions: {
          hostSupported: true,
          loading: false,
          context: {
            saveDir: { path: "/vol1/downloads", displayPath: "存储空间1/下载" },
            filePath: { path: "/vol1/downloads/file.iso", displayPath: "存储空间1/下载/file.iso" },
            actions: {
              availability: "available",
              fileManagerPath: "/vol1/downloads/file.iso",
              openFilePath: "/vol1/downloads/file.iso",
              detailPaths: ["/vol1/downloads/file.iso"],
            },
          },
        },
      },
    });

    expect(wrapper.text()).toContain("存储空间1/下载");
    expect(wrapper.text()).toContain("/vol1/downloads");
    await wrapper.findAll("button").find((button) => button.text() === "打开文件")!.trigger("click");
    await wrapper.findAll("button").find((button) => button.text() === "文件详情")!.trigger("click");
    expect(wrapper.emitted("openFile")).toHaveLength(1);
    expect(wrapper.emitted("showFileDetails")).toHaveLength(1);
  });

  it("renders the summary and detail groups in prototype order", () => {
    const wrapper = mount(TaskDetailsDialog, {
      props: {
        show: true,
        closeLabel: "关闭",
        details: {
          title: "任务详情",
          items: [
            { label: "来源", value: "https://example.com/file.iso" },
            { section: "file", label: "保存路径", value: "存储空间1/下载" },
            { label: "兼容字段", value: "默认归入下载信息" },
          ],
          technicalItems: [{ label: "真实路径", value: "/vol1/downloads/file.iso" }],
        },
        task: createTask({ completedLength: 25, downloadSpeed: 10 }),
        isOperating: false,
        isActionDisabled: false,
      },
    });

    expect(wrapper.get(".task-detail-title").text()).toBe("file.iso");
    expect(wrapper.get('[data-test="task-detail-summary"]').text()).toContain("25.00%");
    expect(wrapper.get('[data-test="task-detail-summary"]').text()).toContain("8s");
    expect(wrapper.findAll("[data-test$='-section']").map((section) => section.attributes("data-test"))).toEqual([
      "task-detail-download-section",
      "task-detail-file-section",
      "task-detail-settings-section",
      "task-detail-technical-section",
    ]);
    expect(wrapper.get('[data-test="task-detail-download-section"]').text()).toContain("默认归入下载信息");
    expect(wrapper.get('[data-test="n-collapse"]').attributes("data-default-expanded-names")).toBe("[]");
  });

  it("shows the host-only hint and disables file actions while loading", async () => {
    const unsupportedWrapper = mount(TaskDetailsDialog, {
      props: {
        show: true,
        closeLabel: "关闭",
        details: { title: "任务详情", items: [] },
        task: createTask({ status: "complete" }),
        isOperating: false,
        isActionDisabled: false,
        fileActions: { hostSupported: false, loading: false, context: null },
      },
    });
    expect(unsupportedWrapper.text()).toContain("文件操作仅支持 fnOS 宿主环境。");

    const loadingWrapper = mount(TaskDetailsDialog, {
      props: {
        show: true,
        closeLabel: "关闭",
        details: { title: "任务详情", items: [] },
        task: createTask({ status: "complete" }),
        isOperating: false,
        isActionDisabled: false,
        fileActions: {
          hostSupported: true,
          loading: true,
          context: {
            saveDir: { path: "/vol1/downloads", displayPath: "存储空间1/下载" },
            filePath: { path: "/vol1/downloads/file.iso", displayPath: "存储空间1/下载/file.iso" },
            actions: {
              availability: "available",
              fileManagerPath: "/vol1/downloads/file.iso",
              openFilePath: "/vol1/downloads/file.iso",
              detailPaths: ["/vol1/downloads/file.iso"],
            },
          },
        },
      },
    });

    const openButton = loadingWrapper.findAll("button").find((button) => button.text() === "打开文件");
    expect(openButton?.attributes("disabled")).toBeDefined();
    expect(openButton?.attributes("data-loading")).toBe("true");
    await openButton?.trigger("click");
    expect(loadingWrapper.emitted("openFile")).toBeUndefined();
  });

  it("copies technical paths through the existing clipboard helper", async () => {
    vi.mocked(copyTextToClipboard).mockClear();
    const wrapper = mount(TaskDetailsDialog, {
      props: {
        show: true,
        closeLabel: "关闭",
        details: {
          title: "任务详情",
          items: [],
          technicalItems: [{ label: "真实路径", value: "/vol1/downloads/file.iso" }],
        },
        task: createTask(),
        isOperating: false,
        isActionDisabled: false,
      },
    });

    await wrapper.findAll("button").find((button) => button.text() === "复制")!.trigger("click");
    expect(copyTextToClipboard).toHaveBeenCalledWith("/vol1/downloads/file.iso");
  });
});

function createTask(overrides: Partial<DownloadTask> = {}): DownloadTask {
  return {
    id: 1,
    url: "https://example.com/file.iso",
    sourceType: "url",
    fileName: "file.iso",
    saveDir: "/downloads",
    category: "默认",
    gid: "gid-1",
    status: "paused",
    totalLength: 100,
    completedLength: 20,
    downloadSpeed: 0,
    errorCode: null,
    errorMessage: null,
    filePath: "/downloads/file.iso",
    useProxy: false,
    confirmationRequired: false,
    files: [],
    createdAt: 1,
    updatedAt: 2,
    ...overrides,
  };
}
