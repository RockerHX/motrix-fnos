import { defineComponent, toRef } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { mountWithPinia, flushPromises } from "../../../test/mount";
import { getAccessiblePaths } from "../../../services/storage";
import { getAppConfig } from "../../../services/settings";
import { createBatchDownloadTasks, createDownloadTask, createTorrentDownloadTask } from "../services/taskService";
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
  createBatchDownloadTasks: vi.fn(),
  createDownloadTask: vi.fn(),
  createTorrentDownloadTask: vi.fn(),
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
const mockedCreateBatchDownloadTasks = vi.mocked(createBatchDownloadTasks);
const mockedCreateDownloadTask = vi.mocked(createDownloadTask);
const mockedCreateTorrentDownloadTask = vi.mocked(createTorrentDownloadTask);

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
      language: "zh-CN",
      jsonRpcToken: "",
    });
  });

  it("requires a valid url before submitting", async () => {
    const { wrapper } = mountHarness();
    await flushPromises();

    wrapper.vm.form.urls = "ftp://example.com/file.iso";
    wrapper.vm.form.saveDir = "/downloads";

    await wrapper.vm.submitCreateTask();

    expect(wrapper.vm.formErrorMessage).toBe("请输入有效的 HTTP / HTTPS 下载链接，并修正无效行");
    expect(mockedCreateBatchDownloadTasks).not.toHaveBeenCalled();
  });

  it("reports invalid URL line numbers and detects valid task count", async () => {
    const { wrapper } = mountHarness();
    await flushPromises();

    wrapper.vm.form.urls = "https://example.com/a.iso\nftp://example.com/b.iso\nhttps://example.com/c.iso";
    await flushPromises();
    expect(wrapper.vm.urlFeedback).toBe("第 2 行不是有效的 HTTP / HTTPS 链接。");
    expect(wrapper.vm.urlValidationStatus).toBe("error");

    wrapper.vm.form.urls = "https://example.com/a.iso\nhttps://example.com/c.iso";
    await flushPromises();
    expect(wrapper.vm.urlFeedback).toBe("检测到 2 个链接，将分别创建 2 个下载任务。");
  });

  it("requires saveDir before submitting", async () => {
    const { wrapper } = mountHarness();
    await flushPromises();

    wrapper.vm.form.urls = "https://example.com/file.iso";
    wrapper.vm.form.saveDir = "";

    await wrapper.vm.submitCreateTask();

    expect(wrapper.vm.formErrorMessage).toBe("请选择已授权的保存目录");
    expect(mockedCreateBatchDownloadTasks).not.toHaveBeenCalled();
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
    mockedCreateBatchDownloadTasks.mockResolvedValueOnce({
      created: [],
      failed: [],
    });

    wrapper.vm.form.urls = "https://example.com/file.iso";
    wrapper.vm.form.saveDir = "/backup";
    wrapper.vm.form.startMode = "paused";
    wrapper.vm.form.category = "电影";
    wrapper.vm.form.connections = 8;
    wrapper.vm.form.downloadLimitKb = 512;
    wrapper.vm.form.proxy = "http://127.0.0.1:7890";

    await wrapper.vm.submitCreateTask();

    expect(mockedCreateBatchDownloadTasks).toHaveBeenCalledWith({
      urls: ["https://example.com/file.iso"],
      saveDir: "/backup",
      startMode: "paused",
      category: "电影",
      advancedOptions: {
        connections: 8,
        downloadLimitKb: 512,
        proxy: "http://127.0.0.1:7890",
      },
    });
    expect(localStorage.getItem("motrix-fnos:last-save-dir")).toBe("/backup");
    expect(wrapper.vm.form.urls).toBe("");
    expect(wrapper.vm.form.saveDir).toBe("");
    expect(wrapper.vm.form.category).toBe("默认");
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onCreated).toHaveBeenCalledTimes(1);
  });

  it("keeps dialog open and displays failures when batch creation partially fails", async () => {
    const { wrapper, onClose, onCreated } = mountHarness();
    await flushPromises();
    mockedCreateBatchDownloadTasks.mockResolvedValueOnce({
      created: [
        {
          id: 101,
          url: "https://example.com/a.iso",
          fileName: "a.iso",
          saveDir: "/downloads",
          category: "默认",
          status: "pending",
          totalLength: 0,
          completedLength: 0,
          downloadSpeed: 0,
          confirmationRequired: false,
          files: [],
          createdAt: 1,
          updatedAt: 1,
        },
      ],
      failed: [{ input: "https://example.com/b.iso", message: "创建 Aria2 下载任务失败" }],
    });

    wrapper.vm.form.urls = " https://example.com/a.iso \n\n https://example.com/b.iso ";
    wrapper.vm.form.saveDir = "/downloads";

    await wrapper.vm.submitCreateTask();

    expect(mockedCreateBatchDownloadTasks).toHaveBeenCalledWith({
      urls: ["https://example.com/a.iso", "https://example.com/b.iso"],
      saveDir: "/downloads",
      startMode: "now",
      category: "默认",
      advancedOptions: {
        connections: 16,
        downloadLimitKb: 0,
        proxy: null,
      },
    });
    expect(wrapper.vm.batchFailedItems).toEqual([
      { input: "https://example.com/b.iso", message: "创建 Aria2 下载任务失败" },
    ]);
    expect(wrapper.vm.formErrorMessage).toBe("已创建部分任务，1 条链接创建失败");
    expect(onClose).not.toHaveBeenCalled();
    expect(onCreated).toHaveBeenCalledTimes(1);
  });

  it("submits magnet task with magnet source type", async () => {
    const { wrapper, onClose, onCreated } = mountHarness();
    await flushPromises();
    mockedCreateDownloadTask.mockResolvedValueOnce({
      id: 102,
      url: "magnet:?xt=urn:btih:test",
      fileName: "磁力链接任务",
      saveDir: "/downloads",
      category: "默认",
      status: "paused",
      totalLength: 0,
      completedLength: 0,
      downloadSpeed: 0,
      confirmationRequired: false,
      files: [],
      createdAt: 1,
      updatedAt: 1,
    });

    wrapper.vm.activeInputType = "magnet";
    wrapper.vm.form.magnet = " magnet:?xt=urn:btih:test ";
    wrapper.vm.form.saveDir = "/downloads";
    wrapper.vm.form.startMode = "paused";

    await wrapper.vm.submitCreateTask();

    expect(mockedCreateDownloadTask).toHaveBeenCalledWith(
      expect.objectContaining({
        url: "magnet:?xt=urn:btih:test",
        fileName: null,
        sourceType: "magnet",
        startMode: "paused",
      }),
    );
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onCreated).toHaveBeenCalledTimes(1);
  });

  it("submits torrent task through store", async () => {
    const { wrapper, onClose, onCreated } = mountHarness();
    await flushPromises();
    const torrent = new File(["torrent"], "example.torrent", { type: "application/x-bittorrent" });
    mockedCreateTorrentDownloadTask.mockResolvedValueOnce({
      id: 103,
      url: "torrent:example.torrent",
      fileName: "example",
      saveDir: "/downloads",
      category: "默认",
      status: "pending",
      totalLength: 0,
      completedLength: 0,
      downloadSpeed: 0,
      confirmationRequired: false,
      files: [],
      createdAt: 1,
      updatedAt: 1,
    });

    wrapper.vm.activeInputType = "torrent";
    wrapper.vm.form.saveDir = "/downloads";
    wrapper.vm.selectTorrentFile(torrent);

    await wrapper.vm.submitCreateTask();

    expect(mockedCreateTorrentDownloadTask).toHaveBeenCalledWith({
      torrent,
      saveDir: "/downloads",
      startMode: "now",
      category: "默认",
      advancedOptions: {
        connections: 16,
        downloadLimitKb: 0,
        proxy: null,
      },
    });
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onCreated).toHaveBeenCalledTimes(1);
  });

  it("warns and aborts submit when runtime is exiting", async () => {
    const { wrapper } = mountHarness();
    await flushPromises();
    const taskStore = useTaskStore();

    taskStore.isRuntimeExiting = true;
    wrapper.vm.form.urls = "https://example.com/file.iso";
    wrapper.vm.form.saveDir = "/downloads";

    await wrapper.vm.submitCreateTask();

    expect(mockMessage.warning).toHaveBeenCalledWith("应用正在退出，请稍候");
    expect(mockedCreateBatchDownloadTasks).not.toHaveBeenCalled();
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
