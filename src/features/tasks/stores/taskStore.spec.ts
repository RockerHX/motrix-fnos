import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../../i18n";
import type { CreateDownloadTaskRequest, DownloadTask } from "../../../types/tasks";
import { useTaskStore } from "./taskStore";
import {
  confirmDownloadTaskFiles,
  createBatchDownloadTasks,
  createDownloadTask,
  createTorrentDownloadTask,
  deleteDownloadTask,
  listDownloadTasks,
  listRemovedDownloadTasks,
  pauseDownloadTask,
  permanentlyDeleteDownloadTask,
  redownloadDownloadTask,
  resumeDownloadTask,
  restoreDownloadTask,
} from "../services/taskService";

vi.mock("../services/taskService", () => ({
  confirmDownloadTaskFiles: vi.fn(),
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
  restoreDownloadTask: vi.fn(),
}));

const mockedConfirmDownloadTaskFiles = vi.mocked(confirmDownloadTaskFiles);
const mockedCreateBatchDownloadTasks = vi.mocked(createBatchDownloadTasks);
const mockedCreateDownloadTask = vi.mocked(createDownloadTask);
const mockedCreateTorrentDownloadTask = vi.mocked(createTorrentDownloadTask);
const mockedDeleteDownloadTask = vi.mocked(deleteDownloadTask);
const mockedListDownloadTasks = vi.mocked(listDownloadTasks);
const mockedListRemovedDownloadTasks = vi.mocked(listRemovedDownloadTasks);
const mockedPauseDownloadTask = vi.mocked(pauseDownloadTask);
const mockedPermanentlyDeleteDownloadTask = vi.mocked(permanentlyDeleteDownloadTask);
const mockedRedownloadDownloadTask = vi.mocked(redownloadDownloadTask);
const mockedResumeDownloadTask = vi.mocked(resumeDownloadTask);
const mockedRestoreDownloadTask = vi.mocked(restoreDownloadTask);

describe("taskStore refresh and operation state", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    vi.useRealTimers();
  });

  it("refreshTasks success writes latest tasks", async () => {
    const store = useTaskStore();
    const tasks = [createTask({ id: 1, status: "active" })];
    mockedListDownloadTasks.mockResolvedValueOnce(tasks);

    const result = await store.refreshTasks();

    expect(result).toEqual({ taskErrorMessages: [] });
    expect(store.tasks).toEqual(tasks);
    expect(store.isRefreshing).toBe(false);
  });

  it("refreshTasks failure throttles error reporting unless showError is enabled", async () => {
    const store = useTaskStore();
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-06T00:00:00Z"));
    mockedListDownloadTasks.mockRejectedValue(new Error("refresh failed"));

    await expect(store.refreshTasks()).resolves.toEqual({
      refreshError: "refresh failed",
      taskErrorMessages: [],
    });

    await expect(store.refreshTasks()).resolves.toEqual({
      taskErrorMessages: [],
    });

    vi.setSystemTime(new Date("2026-07-06T00:00:05Z"));
    await expect(store.refreshTasks({ showError: true })).resolves.toEqual({
      refreshError: "refresh failed",
      taskErrorMessages: [],
    });
  });

  it("refreshRemovedTasks success and failure update removed task list", async () => {
    const store = useTaskStore();
    const removedTasks = [createTask({ id: 9, status: "removed" })];
    mockedListRemovedDownloadTasks.mockResolvedValueOnce(removedTasks);

    await expect(store.refreshRemovedTasks()).resolves.toEqual({
      taskErrorMessages: [],
    });
    expect(store.removedTasks).toEqual(removedTasks);

    mockedListRemovedDownloadTasks.mockRejectedValueOnce(new Error("removed failed"));
    await expect(store.refreshRemovedTasks({ showError: true })).resolves.toEqual({
      refreshError: "removed failed",
      taskErrorMessages: [],
    });
  });

  it("createTask toggles isCreating while request is in flight", async () => {
    const store = useTaskStore();
    const payload: CreateDownloadTaskRequest = {
      url: "https://example.com/new.iso",
      fileName: "new.iso",
      saveDir: "/downloads",
    };
    const createdTask = createTask({ id: 11, fileName: "new.iso" });
    const deferred = createDeferred<DownloadTask>();
    mockedCreateDownloadTask.mockReturnValueOnce(deferred.promise);

    const promise = store.createTask(payload);
    expect(store.isCreating).toBe(true);

    deferred.resolve(createdTask);
    await expect(promise).resolves.toEqual(createdTask);
    expect(store.isCreating).toBe(false);
    expect(store.tasks[0]).toEqual(createdTask);
  });

  it("createBatchTasks toggles isCreating and inserts created tasks", async () => {
    const store = useTaskStore();
    const createdTasks = [createTask({ id: 12, fileName: "a.iso" }), createTask({ id: 13, fileName: "b.iso" })];
    const deferred = createDeferred<{
      created: DownloadTask[];
      failed: Array<{ input: string; message: string }>;
    }>();
    mockedCreateBatchDownloadTasks.mockReturnValueOnce(deferred.promise);

    const promise = store.createBatchTasks({
      urls: ["https://example.com/a.iso", "https://example.com/b.iso"],
      saveDir: "/downloads",
    });
    expect(store.isCreating).toBe(true);

    deferred.resolve({
      created: createdTasks,
      failed: [{ input: "ftp://example.com/c.iso", message: "当前仅支持 HTTP / HTTPS 下载链接" }],
    });
    await expect(promise).resolves.toEqual({
      created: createdTasks,
      failed: [{ input: "ftp://example.com/c.iso", message: "当前仅支持 HTTP / HTTPS 下载链接" }],
    });
    expect(store.isCreating).toBe(false);
    expect(store.tasks.map((task) => task.id)).toEqual([12, 13]);
  });

  it("createTorrentTask toggles isCreating and inserts created task", async () => {
    const store = useTaskStore();
    const createdTask = createTask({ id: 14, fileName: "movie" });
    const deferred = createDeferred<DownloadTask>();
    const torrent = new File(["torrent"], "movie.torrent", { type: "application/x-bittorrent" });
    mockedCreateTorrentDownloadTask.mockReturnValueOnce(deferred.promise);

    const promise = store.createTorrentTask({
      torrent,
      saveDir: "/downloads",
      startMode: "paused",
      category: "电影",
      advancedOptions: {
        connections: 8,
        downloadLimitKb: 512,
        proxy: "http://127.0.0.1:7890",
      },
    });
    expect(store.isCreating).toBe(true);

    deferred.resolve(createdTask);
    await expect(promise).resolves.toEqual(createdTask);
    expect(store.isCreating).toBe(false);
    expect(store.tasks[0]).toEqual(createdTask);
  });

  it("task operations toggle operating ids and update task collections", async () => {
    const store = useTaskStore();
    const activeTask = createTask({ id: 21, status: "active" });
    const pausedTask = createTask({ id: 21, status: "paused" });
    const resumedTask = createTask({ id: 21, status: "active" });
    const completedTask = createTask({ id: 22, status: "complete" });
    const redownloadedTask = createTask({ id: 22, status: "pending" });
    const removedTask = createTask({ id: 22, status: "removed" });

    store.tasks = [activeTask, completedTask];

    mockedPauseDownloadTask.mockResolvedValueOnce(pausedTask);
    const pausePromise = store.pauseTask(activeTask.id);
    expect(store.isTaskOperating(activeTask.id)).toBe(true);
    await expect(pausePromise).resolves.toEqual(pausedTask);
    expect(store.isTaskOperating(activeTask.id)).toBe(false);
    expect(store.tasks[0]).toEqual(pausedTask);

    mockedResumeDownloadTask.mockResolvedValueOnce(resumedTask);
    await expect(store.resumeTask(activeTask.id)).resolves.toEqual(resumedTask);
    expect(store.tasks[0]).toEqual(resumedTask);

    const confirmationTask = createTask({
      id: 23,
      status: "paused",
      confirmationRequired: true,
      files: [
        {
          index: 1,
          path: "/downloads/movie/file-a.mkv",
          name: "file-a.mkv",
          length: 1024,
          completedLength: 0,
          selected: true,
        },
      ],
    });
    const confirmedTask = createTask({
      ...confirmationTask,
      status: "active",
      confirmationRequired: false,
    });
    store.tasks = [...store.tasks, confirmationTask];
    mockedConfirmDownloadTaskFiles.mockResolvedValueOnce(confirmedTask);
    await expect(store.confirmTaskFiles(confirmationTask.id, [1])).resolves.toEqual(confirmedTask);
    expect(mockedConfirmDownloadTaskFiles).toHaveBeenCalledWith(confirmationTask.id, {
      selectedFileIndexes: [1],
    });
    expect(store.tasks.find((task) => task.id === confirmationTask.id)).toEqual(confirmedTask);

    mockedRedownloadDownloadTask.mockResolvedValueOnce(redownloadedTask);
    await expect(store.redownloadTask(completedTask.id)).resolves.toEqual(redownloadedTask);
    expect(store.tasks.find((task) => task.id === completedTask.id)).toEqual(redownloadedTask);

    mockedDeleteDownloadTask.mockResolvedValueOnce(removedTask);
    await expect(store.deleteTask(completedTask.id, true)).resolves.toEqual(removedTask);
    expect(store.tasks.find((task) => task.id === completedTask.id)).toBeUndefined();
  });

  it("permanentlyDeleteTask removes removed task after request succeeds", async () => {
    const store = useTaskStore();
    const removedTask = createTask({ id: 31, status: "removed" });
    const deferred = createDeferred<void>();

    store.removedTasks = [removedTask];
    mockedPermanentlyDeleteDownloadTask.mockReturnValueOnce(deferred.promise);

    const promise = store.permanentlyDeleteTask(removedTask.id);
    expect(store.isTaskOperating(removedTask.id)).toBe(true);

    deferred.resolve();
    await expect(promise).resolves.toBeUndefined();
    expect(store.isTaskOperating(removedTask.id)).toBe(false);
    expect(store.removedTasks).toEqual([]);
  });

  it("restoreTask moves a removed task back to the active collection", async () => {
    const store = useTaskStore();
    const removedTask = createTask({ id: 32, status: "removed" });
    const restoredTask = createTask({ id: 32, status: "paused", gid: "restored-gid" });
    store.removedTasks = [removedTask];
    mockedRestoreDownloadTask.mockResolvedValueOnce(restoredTask);

    const promise = store.restoreTask(removedTask.id);
    expect(store.isTaskOperating(removedTask.id)).toBe(true);
    await expect(promise).resolves.toEqual(restoredTask);

    expect(store.isTaskOperating(removedTask.id)).toBe(false);
    expect(store.removedTasks).toEqual([]);
    expect(store.tasks).toContainEqual(restoredTask);
  });

  it("restoreTask keeps the removed task when the request fails", async () => {
    const store = useTaskStore();
    const removedTask = createTask({ id: 33, status: "removed" });
    store.removedTasks = [removedTask];
    mockedRestoreDownloadTask.mockRejectedValueOnce(new Error("restore failed"));

    await expect(store.restoreTask(removedTask.id)).rejects.toThrow("restore failed");

    expect(store.removedTasks).toEqual([removedTask]);
    expect(store.tasks).toEqual([]);
    expect(store.isTaskOperating(removedTask.id)).toBe(false);
  });
});

describe("taskStore snapshot and runtime exiting", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    vi.useRealTimers();
  });

  it("does not report historical error tasks on first load but reports new errors on later snapshots", async () => {
    const store = useTaskStore();
    const existingErrorTask = createTask({
      id: 41,
      gid: "gid-41",
      status: "error",
      errorCode: "3",
      errorMessage: "disk full",
    });
    const newErrorTask = createTask({
      id: 42,
      gid: "gid-42",
      status: "error",
      errorCode: "5",
      errorMessage: "network lost",
    });

    mockedListDownloadTasks.mockResolvedValueOnce([existingErrorTask]);
    await store.refreshTasks();
    expect(store.consumeTaskErrorMessages()).toEqual([]);

    store.applyTaskSnapshot({
      tasks: [existingErrorTask, newErrorTask],
    });

    const messages = store.consumeTaskErrorMessages();
    expect(messages).toHaveLength(1);
    expect(messages[0]).toContain("network lost");
    expect(store.consumeTaskErrorMessages()).toEqual([]);
  });

  it("applyTaskSnapshot replaces task list when runtime is active", () => {
    const store = useTaskStore();
    const snapshotTasks = [createTask({ id: 51, status: "complete" })];

    store.applyTaskSnapshot({ tasks: snapshotTasks });

    expect(store.tasks).toEqual(snapshotTasks);
  });

  it("markRuntimeExiting sets exit state and default reason", () => {
    const store = useTaskStore();

    store.markRuntimeExiting({
      reason: "",
      timestamp: Date.now(),
    });

    expect(store.isRuntimeExiting).toBe(true);
    expect(store.runtimeExitReason).toBe(t("task.runtimeExiting"));
  });

  it("runtime exiting prevents refreshes and ignores future snapshots", async () => {
    const store = useTaskStore();
    const nextTasks = [createTask({ id: 61, status: "active" })];

    store.tasks = [createTask({ id: 60, status: "pending" })];
    store.markRuntimeExiting({
      reason: "shutting down",
      timestamp: Date.now(),
    });

    mockedListDownloadTasks.mockResolvedValue(nextTasks);
    mockedListRemovedDownloadTasks.mockResolvedValue([createTask({ id: 62, status: "removed" })]);

    await expect(store.refreshTasks()).resolves.toEqual({ taskErrorMessages: [] });
    await expect(store.refreshRemovedTasks()).resolves.toEqual({ taskErrorMessages: [] });

    store.applyTaskSnapshot({ tasks: nextTasks });

    expect(mockedListDownloadTasks).not.toHaveBeenCalled();
    expect(mockedListRemovedDownloadTasks).not.toHaveBeenCalled();
    expect(store.tasks).toEqual([createTask({ id: 60, status: "pending" })]);
  });

  it("runtime exiting rejects create and operation calls", async () => {
    const store = useTaskStore();
    const removedTask = createTask({ id: 71, status: "removed" });

    store.removedTasks = [removedTask];
    store.markRuntimeExiting({
      reason: "shutting down",
      timestamp: Date.now(),
    });

    await expect(
      store.createTask({
        url: "https://example.com/new.iso",
        fileName: "new.iso",
        saveDir: "/downloads",
      }),
    ).rejects.toThrow(t("task.runtimeExiting"));

    await expect(store.pauseTask(71)).rejects.toThrow(t("task.runtimeExiting"));
    await expect(store.restoreTask(removedTask.id)).rejects.toThrow(t("task.runtimeExiting"));
    await expect(store.permanentlyDeleteTask(removedTask.id)).rejects.toThrow(t("task.runtimeExiting"));

    expect(mockedCreateDownloadTask).not.toHaveBeenCalled();
    expect(mockedCreateBatchDownloadTasks).not.toHaveBeenCalled();
    expect(mockedCreateTorrentDownloadTask).not.toHaveBeenCalled();
    expect(mockedConfirmDownloadTaskFiles).not.toHaveBeenCalled();
    expect(mockedPauseDownloadTask).not.toHaveBeenCalled();
    expect(mockedPermanentlyDeleteDownloadTask).not.toHaveBeenCalled();
    expect(mockedRestoreDownloadTask).not.toHaveBeenCalled();
  });
});

function createTask(overrides: Partial<DownloadTask> = {}): DownloadTask {
  return {
    id: 1,
    url: "https://example.com/file.iso",
    fileName: "file.iso",
    saveDir: "/downloads",
    category: "默认",
    gid: "gid-1",
    status: "pending",
    totalLength: 1024,
    completedLength: 0,
    downloadSpeed: 128,
    errorCode: null,
    errorMessage: null,
    filePath: null,
    confirmationRequired: false,
    files: [],
    createdAt: 1,
    updatedAt: 1,
    ...overrides,
  };
}

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });

  return {
    promise,
    resolve,
    reject,
  };
}
