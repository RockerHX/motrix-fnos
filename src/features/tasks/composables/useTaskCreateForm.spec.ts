import { defineComponent, toRef } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { mountWithPinia, flushPromises } from "../../../test/mount";
import { getAccessiblePaths } from "../../../services/storage";
import { getAppConfig } from "../../../services/settings";
import { createDownloadTask } from "../services/taskService";
import { useTaskStore } from "../stores/taskStore";
import { useTaskCreateForm } from "./useTaskCreateForm";

const mockMessage = {
  warning: vi.fn(),
  error: vi.fn(),
};

vi.mock("naive-ui", async (importOriginal) => {
  const actual = await importOriginal<typeof import("naive-ui")>();
  return {
    ...actual,
    useMessage: () => mockMessage,
  };
});

vi.mock("../../../services/storage", () => ({
  getAccessiblePaths: vi.fn(),
}));

vi.mock("../../../services/settings", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../../services/settings")>();
  return {
    ...actual,
    getAppConfig: vi.fn(),
  };
});

vi.mock("../services/taskService", () => ({
  createDownloadTask: vi.fn(),
  deleteDownloadTask: vi.fn(),
  listDownloadTasks: vi.fn(),
  listRemovedDownloadTasks: vi.fn(),
  pauseDownloadTask: vi.fn(),
  permanentlyDeleteDownloadTask: vi.fn(),
  redownloadDownloadTask: vi.fn(),
  resumeDownloadTask: vi.fn(),
}));

const mockedGetAccessiblePaths = vi.mocked(getAccessiblePaths);
const mockedGetAppConfig = vi.mocked(getAppConfig);
const mockedCreateDownloadTask = vi.mocked(createDownloadTask);

describe("useTaskCreateForm", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
    setActivePinia(createPinia());
    mockedGetAccessiblePaths.mockResolvedValue({
      paths: ["/downloads", "/backup"],
    });
    mockedGetAppConfig.mockResolvedValue({
      defaultDownloadDir: "/downloads",
      maxConcurrentDownloads: 3,
      downloadLimit: 0,
      uploadLimit: 0,
      autoStartEnabled: false,
      notificationsEnabled: false,
      language: "zh-CN",
      jsonRpcToken: "",
    });
  });

  it("requires a valid url before submitting", async () => {
    const { wrapper } = mountHarness();
    await flushPromises();

    wrapper.vm.form.url = "ftp://example.com/file.iso";
    wrapper.vm.form.saveDir = "/downloads";

    await wrapper.vm.submitCreateTask();

    expect(wrapper.vm.formErrorMessage).toBe("请输入有效的 HTTP / HTTPS 下载链接");
    expect(mockedCreateDownloadTask).not.toHaveBeenCalled();
  });

  it("requires saveDir before submitting", async () => {
    const { wrapper } = mountHarness();
    await flushPromises();

    wrapper.vm.form.url = "https://example.com/file.iso";
    wrapper.vm.form.saveDir = "";

    await wrapper.vm.submitCreateTask();

    expect(wrapper.vm.formErrorMessage).toBe("请选择已授权的保存目录");
    expect(mockedCreateDownloadTask).not.toHaveBeenCalled();
  });

  it("prefers default dir, then remembered dir, then first accessible path", async () => {
    const { wrapper: defaultWrapper } = mountHarness();
    await flushPromises();
    expect(defaultWrapper.vm.form.saveDir).toBe("/downloads");

    localStorage.setItem("motrix-fnos:last-save-dir", "/backup");
    mockedGetAppConfig.mockResolvedValueOnce({
      defaultDownloadDir: "/missing",
      maxConcurrentDownloads: 3,
      downloadLimit: 0,
      uploadLimit: 0,
      autoStartEnabled: false,
      notificationsEnabled: false,
      language: "zh-CN",
      jsonRpcToken: "",
    });
    const { wrapper: rememberedWrapper } = mountHarness();
    await flushPromises();
    expect(rememberedWrapper.vm.form.saveDir).toBe("/backup");

    localStorage.removeItem("motrix-fnos:last-save-dir");
    mockedGetAppConfig.mockResolvedValueOnce({
      defaultDownloadDir: "/missing",
      maxConcurrentDownloads: 3,
      downloadLimit: 0,
      uploadLimit: 0,
      autoStartEnabled: false,
      notificationsEnabled: false,
      language: "zh-CN",
      jsonRpcToken: "",
    });
    const { wrapper: fallbackWrapper } = mountHarness({
      accessiblePaths: ["/first", "/second"],
    });
    await flushPromises();
    expect(fallbackWrapper.vm.form.saveDir).toBe("/first");
  });

  it("stores accessible path load failure", async () => {
    mockedGetAccessiblePaths.mockRejectedValueOnce(new Error("load failed"));
    const { wrapper } = mountHarness();
    await flushPromises();

    expect(wrapper.vm.accessiblePathsError).toBe("load failed");
    expect(wrapper.vm.accessiblePaths).toEqual([]);
    expect(wrapper.vm.form.saveDir).toBe("");
  });

  it("submits successfully, remembers saveDir and resets the form", async () => {
    const { wrapper, onClose, onCreated } = mountHarness();
    await flushPromises();
    mockedCreateDownloadTask.mockResolvedValueOnce({
      id: 100,
      url: "https://example.com/file.iso",
      fileName: "custom.iso",
      saveDir: "/backup",
      status: "pending",
      totalLength: 1024,
      completedLength: 0,
      downloadSpeed: 0,
      createdAt: 1,
      updatedAt: 1,
    });

    wrapper.vm.form.url = "https://example.com/file.iso";
    wrapper.vm.form.fileName = "custom.iso";
    wrapper.vm.form.saveDir = "/backup";
    wrapper.vm.form.note = "keep";

    await wrapper.vm.submitCreateTask();

    expect(mockedCreateDownloadTask).toHaveBeenCalledWith({
      url: "https://example.com/file.iso",
      fileName: "custom.iso",
      saveDir: "/backup",
    });
    expect(localStorage.getItem("motrix-fnos:last-save-dir")).toBe("/backup");
    expect(wrapper.vm.form.url).toBe("");
    expect(wrapper.vm.form.fileName).toBe("");
    expect(wrapper.vm.form.saveDir).toBe("");
    expect(wrapper.vm.form.note).toBe("");
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onCreated).toHaveBeenCalledTimes(1);
  });

  it("warns and aborts submit when runtime is exiting", async () => {
    const { wrapper } = mountHarness();
    await flushPromises();
    const taskStore = useTaskStore();

    taskStore.isRuntimeExiting = true;
    wrapper.vm.form.url = "https://example.com/file.iso";
    wrapper.vm.form.saveDir = "/downloads";

    await wrapper.vm.submitCreateTask();

    expect(mockMessage.warning).toHaveBeenCalledWith("应用正在退出，请稍候");
    expect(mockedCreateDownloadTask).not.toHaveBeenCalled();
  });
});

function mountHarness(options: { accessiblePaths?: string[] } = {}) {
  if (options.accessiblePaths) {
    mockedGetAccessiblePaths.mockResolvedValueOnce({
      paths: options.accessiblePaths,
    });
  }

  const onClose = vi.fn();
  const onCreated = vi.fn();
  const Harness = defineComponent({
    props: {
      show: {
        type: Boolean,
        default: true,
      },
    },
    setup(props) {
      return {
        ...useTaskCreateForm({
          show: toRef(props, "show"),
          onClose,
          onCreated,
        }),
      };
    },
    template: "<div />",
  });

  const { wrapper } = mountWithPinia(Harness);

  return {
    wrapper,
    onClose,
    onCreated,
  };
}
