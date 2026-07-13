import { reactive, ref } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createTaskCreateFormState, type TaskCreateInputType } from "./taskCreateFormModel";
import { useTaskCreateSubmission } from "./useTaskCreateSubmission";
import { useTaskCreateValidation } from "./useTaskCreateValidation";

const message = vi.hoisted(() => ({ warning: vi.fn(), error: vi.fn() }));
const taskStore = vi.hoisted(() => ({
  isRuntimeExiting: false,
  isCreating: false,
  createBatchTasks: vi.fn(),
  createTorrentTask: vi.fn(),
  createTask: vi.fn(),
}));

vi.mock("naive-ui", () => ({ useMessage: () => message }));
vi.mock("../stores/taskStore", () => ({ useTaskStore: () => taskStore }));

describe("useTaskCreateSubmission", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    taskStore.isRuntimeExiting = false;
    taskStore.createBatchTasks.mockResolvedValue({ created: [], failed: [] });
    taskStore.createTorrentTask.mockResolvedValue(undefined);
    taskStore.createTask.mockResolvedValue(undefined);
  });

  it("submits URL, torrent and magnet sources through their store methods", async () => {
    const setup = createSubmission();
    setup.form.urls = "https://example.com/file.iso";
    setup.form.saveDir = "/downloads";
    await setup.submission.submitCreateTask();
    expect(taskStore.createBatchTasks).toHaveBeenCalledOnce();

    setup.activeInputType.value = "torrent";
    setup.form.torrentFile = new File(["torrent"], "example.torrent");
    setup.form.saveDir = "/downloads";
    await setup.submission.submitCreateTask();
    expect(taskStore.createTorrentTask).toHaveBeenCalledOnce();

    setup.activeInputType.value = "magnet";
    setup.form.magnet = " magnet:?xt=urn:btih:test ";
    setup.form.saveDir = "/downloads";
    await setup.submission.submitCreateTask();
    expect(taskStore.createTask).toHaveBeenCalledWith(expect.objectContaining({ url: "magnet:?xt=urn:btih:test" }));
  });

  it("keeps the dialog open and records partial batch failures", async () => {
    taskStore.createBatchTasks.mockResolvedValueOnce({
      created: [{}],
      failed: [{ input: "https://example.com/b.iso", message: "failed" }],
    });
    const setup = createSubmission();
    setup.form.urls = "https://example.com/a.iso\nhttps://example.com/b.iso";
    setup.form.saveDir = "/downloads";

    await setup.submission.submitCreateTask();

    expect(setup.submission.formErrorMessage.value).toBe("已创建部分任务，1 条链接创建失败");
    expect(setup.onClose).not.toHaveBeenCalled();
    expect(setup.onCreated).toHaveBeenCalledOnce();
  });

  it("warns and aborts while the runtime is exiting", async () => {
    taskStore.isRuntimeExiting = true;
    const setup = createSubmission();
    setup.form.urls = "https://example.com/file.iso";
    setup.form.saveDir = "/downloads";

    await setup.submission.submitCreateTask();

    expect(message.warning).toHaveBeenCalledWith("应用正在退出，请稍候");
    expect(taskStore.createBatchTasks).not.toHaveBeenCalled();
  });
});

function createSubmission() {
  const form = reactive(createTaskCreateFormState());
  const activeInputType = ref<TaskCreateInputType>("url");
  const onClose = vi.fn();
  const onCreated = vi.fn();
  const submission = useTaskCreateSubmission({
    form,
    activeInputType,
    validation: useTaskCreateValidation(form, activeInputType),
    rememberSaveDir: vi.fn(),
    resetForm: vi.fn(),
    onClose,
    onCreated,
  });
  return { form, activeInputType, submission, onClose, onCreated };
}
