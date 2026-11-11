import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import { createTask } from "../../../test/tasks";
import { useTaskStore } from "../stores/taskStore";
import { getTaskFileContext } from "../services/taskService";
import { setLanguage } from "../../../i18n";
import type { TaskFileAvailability, TaskFileContextResponse } from "../../../types/tasks";
import TaskActionsContainer from "./TaskActionsContainer.vue";
import TaskActions from "./TaskActions.vue";

const messages = vi.hoisted(() => ({ success: vi.fn(), warning: vi.fn(), error: vi.fn() }));
vi.mock("naive-ui", async (original) => ({
  ...await original<typeof import("naive-ui")>(),
  useMessage: () => messages,
}));
vi.mock("../services/taskService", () => ({ getTaskFileContext: vi.fn() }));
vi.mock("../../../services/fnos", () => ({ fnosHost: { getHostKind: async () => "unavailable" } }));

function context(availability: TaskFileAvailability): TaskFileContextResponse {
  return {
    saveDir: { path: "/downloads", displayPath: "/downloads" },
    filePath: null,
    actions: { availability, fileManagerPath: null, openFilePath: null, detailPaths: [] },
  };
}

function mountActions(mode: "desktop" | "mobile" = "desktop", status = "complete" as "complete" | "paused") {
  return mountWithPinia(TaskActionsContainer, {
    props: { mode, task: createTask({ status }) },
    global: { stubs: { TaskActions: true, TaskFileConfirmDialog: true } },
  });
}

describe("TaskActionsContainer local files", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    setLanguage("zh-CN");
    vi.mocked(getTaskFileContext).mockResolvedValue(context("available"));
  });

  it.each(["desktop", "mobile"] as const)("keeps completion history and checks missing files before %s redownload", async (mode) => {
    const { wrapper } = mountActions(mode);
    await flushPromises();
    const actions = wrapper.getComponent(TaskActions);
    expect(wrapper.get('[role="status"]').text()).toContain("文件已准备就绪");
    vi.mocked(getTaskFileContext).mockResolvedValue(context("path_missing"));
    expect(await actions.props("prepareRedownload")!()).toBe(true);
    expect(getTaskFileContext).toHaveBeenCalledTimes(2);
    expect(wrapper.get('[role="status"]').text()).toContain("本地文件不存在");
    expect(actions.props("task").status).toBe("complete");
    expect(actions.props("confirmTexts").redownloadConfirmText).toContain("从 0 开始");
    expect(actions.props("permissions").canResume).toBe(false);
    expect(actions.props("permissions").canRedownload).toBe(true);
    wrapper.unmount();
  });

  it.each(["storage_unavailable", "path_unauthorized", "unsupported_layout"] as const)("blocks redownload for %s without claiming files were deleted", async (availability) => {
    vi.mocked(getTaskFileContext).mockResolvedValue(context(availability));
    const { wrapper } = mountActions();
    await flushPromises();
    expect(await wrapper.getComponent(TaskActions).props("prepareRedownload")!()).toBe(false);
    expect(messages.warning).toHaveBeenCalledOnce();
    expect(wrapper.get('[role="status"]').text()).not.toContain("不存在");
    wrapper.unmount();
  });

  it("explains that partial loss redownload clears remaining files instead of repairing them", async () => {
    vi.mocked(getTaskFileContext).mockResolvedValue(context("partial_missing"));
    const { wrapper } = mountActions();
    await flushPromises();
    expect(wrapper.text()).toContain("部分本地文件缺失");
    expect(wrapper.getComponent(TaskActions).props("confirmTexts").redownloadConfirmText)
      .toContain("清理剩余文件");
    setLanguage("en-US");
    await flushPromises();
    expect(wrapper.getComponent(TaskActions).props("confirmTexts").redownloadConfirmText)
      .toContain("rather than only replace missing files");
    wrapper.unmount();
  });

  it("refreshes local status with the existing task refresh", async () => {
    const { wrapper, pinia } = mountActions();
    await flushPromises();
    const store = useTaskStore(pinia);
    store.isRefreshing = true;
    await flushPromises();
    vi.mocked(getTaskFileContext).mockResolvedValue(context("path_missing"));
    store.isRefreshing = false;
    await flushPromises();
    expect(wrapper.text()).toContain("本地文件不存在");
    wrapper.unmount();
  });

  it("ignores late responses for a replaced task", async () => {
    let resolveOld!: (value: TaskFileContextResponse) => void;
    vi.mocked(getTaskFileContext).mockReturnValueOnce(new Promise((resolve) => { resolveOld = resolve; }));
    const { wrapper } = mountActions();
    await wrapper.setProps({ task: createTask({ id: 2, status: "complete" }) });
    await flushPromises();
    resolveOld(context("path_missing"));
    await flushPromises();
    expect(wrapper.get('[role="status"]').text()).toContain("文件已准备就绪");
    wrapper.unmount();
  });

  it("fails closed on a context error and leaves unfinished tasks alone", async () => {
    vi.mocked(getTaskFileContext).mockRejectedValue(new Error("offline"));
    const { wrapper } = mountActions();
    await flushPromises();
    expect(wrapper.text()).toContain("无法检查本地文件");
    expect(await wrapper.getComponent(TaskActions).props("prepareRedownload")!()).toBe(false);
    expect(messages.error).toHaveBeenCalledOnce();
    wrapper.unmount();
    vi.mocked(getTaskFileContext).mockClear();
    const paused = mountActions("desktop", "paused").wrapper;
    expect(getTaskFileContext).not.toHaveBeenCalled();
    expect(paused.find('[role="status"]').exists()).toBe(false);
    paused.unmount();
  });
});
