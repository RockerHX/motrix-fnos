import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CreateDownloadTaskRequest, DownloadTask } from "../../../types/tasks";
import { useTaskStore } from "./taskStore";
import {
  createDownloadTask,
  deleteDownloadTask,
  listDownloadTasks,
  listRemovedDownloadTasks,
  pauseDownloadTask,
  permanentlyDeleteDownloadTask,
  redownloadDownloadTask,
  resumeDownloadTask,
} from "../services/taskService";

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

const mockedCreateDownloadTask = vi.mocked(createDownloadTask);
const mockedDeleteDownloadTask = vi.mocked(deleteDownloadTask);
const mockedListDownloadTasks = vi.mocked(listDownloadTasks);
const mockedListRemovedDownloadTasks = vi.mocked(listRemovedDownloadTasks);
const mockedPauseDownloadTask = vi.mocked(pauseDownloadTask);
const mockedPermanentlyDeleteDownloadTask = vi.mocked(permanentlyDeleteDownloadTask);
const mockedRedownloadDownloadTask = vi.mocked(redownloadDownloadTask);
const mockedResumeDownloadTask = vi.mocked(resumeDownloadTask);

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
});

function createTask(overrides: Partial<DownloadTask> = {}): DownloadTask {
  return {
    id: 1,
    url: "https://example.com/file.iso",
    fileName: "file.iso",
    saveDir: "/downloads",
    gid: "gid-1",
    status: "pending",
    totalLength: 1024,
    completedLength: 0,
    downloadSpeed: 128,
    errorCode: null,
    errorMessage: null,
    filePath: null,
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
