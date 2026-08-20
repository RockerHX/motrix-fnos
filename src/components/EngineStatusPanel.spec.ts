import { nextTick } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import EngineStatusPanel from "./EngineStatusPanel.vue";
import { flushPromises, mountWithPinia } from "../test/mount";
import {
  getAria2ConfigStatus,
  getAria2ProcessStatus,
  pingAria2Rpc,
  startAria2,
  stopAria2,
} from "../services/aria2";
import type { Aria2ConfigStatus, Aria2ProcessStatus, Aria2RpcStatus } from "../types/aria2";

vi.mock("../services/aria2", () => ({
  getAria2ConfigStatus: vi.fn(),
  getAria2ProcessStatus: vi.fn(),
  pingAria2Rpc: vi.fn(),
  startAria2: vi.fn(),
  stopAria2: vi.fn(),
}));

const mockGetConfig = vi.mocked(getAria2ConfigStatus);
const mockGetProcess = vi.mocked(getAria2ProcessStatus);
const mockPingRpc = vi.mocked(pingAria2Rpc);
const mockStart = vi.mocked(startAria2);
const mockStop = vi.mocked(stopAria2);

describe("EngineStatusPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetConfig.mockResolvedValue(createConfigStatus());
    mockGetProcess.mockResolvedValue(createProcessStatus({ running: false, message: "未启动" }));
    mockPingRpc.mockResolvedValue(createRpcStatus({ connected: false, message: "未连接" }));
    mockStart.mockResolvedValue(createProcessStatus({ running: true, pid: 1234, message: "已启动" }));
    mockStop.mockResolvedValue(createProcessStatus({ running: false, message: "已停止" }));
  });

  it("refreshes engine status on mount and emits statusUpdated", async () => {
    const { wrapper } = mountWithPinia(EngineStatusPanel);

    await flushPromises();

    expect(mockGetConfig).toHaveBeenCalledTimes(1);
    expect(mockGetProcess).toHaveBeenCalledTimes(1);
    expect(mockPingRpc).toHaveBeenCalledTimes(1);
    expect(wrapper.emitted("statusUpdated")?.[0]).toEqual([
      {
        process: createProcessStatus({ running: false, message: "未启动" }),
        rpc: createRpcStatus({ connected: false, message: "未连接" }),
      },
    ]);
    expect(wrapper.text()).toContain("Aria2 Next");
    expect(wrapper.text()).toContain("未启动");
  });

  it("runs start, stop and rpc actions from Naive UI buttons", async () => {
    const { wrapper } = mountWithPinia(EngineStatusPanel);
    await flushPromises();

    await clickButton(wrapper, "启动引擎");
    await clickButton(wrapper, "停止引擎");
    await clickButton(wrapper, "检查 RPC");

    expect(mockStart).toHaveBeenCalledTimes(1);
    expect(mockStop).toHaveBeenCalledTimes(1);
    expect(mockPingRpc).toHaveBeenCalledTimes(5);
  });

  it("shows loading state while an engine action is running", async () => {
    const startDeferred = createDeferred<Aria2ProcessStatus>();
    mockStart.mockReturnValueOnce(startDeferred.promise);

    const { wrapper } = mountWithPinia(EngineStatusPanel);
    await flushPromises();

    await clickButton(wrapper, "启动引擎", false);
    await nextTick();

    expect(wrapper.findAll("button").every((button) => button.attributes("disabled") !== undefined)).toBe(true);

    startDeferred.resolve(createProcessStatus({ running: true, pid: 1234, message: "已启动" }));
    await flushPromises();

    expect(wrapper.findAll("button").every((button) => button.attributes("disabled") === undefined)).toBe(true);
  });

  it("shows lifecycle completion, busy and fallback failure feedback", async () => {
    const { wrapper } = mountWithPinia(EngineStatusPanel);
    await flushPromises();

    await clickButton(wrapper, "启动引擎");
    expect(wrapper.text()).toContain("引擎已启动");

    mockStop.mockRejectedValueOnce(new Error("Aria2 正在停止，请稍后重试"));
    await clickButton(wrapper, "停止引擎");
    expect(wrapper.text()).toContain("Aria2 正在停止，请稍后重试");
    expect(wrapper.text()).not.toContain("引擎已停止");

    mockStart.mockRejectedValueOnce(new Error());
    await clickButton(wrapper, "启动引擎");
    expect(wrapper.text()).toContain("引擎操作失败，请稍后重试");
  });
});

async function clickButton(wrapper: ReturnType<typeof mountWithPinia>["wrapper"], text: string, waitForFlush = true) {
  const button = wrapper.findAll("button").find((item) => item.text() === text);
  expect(button, `button ${text} should exist`).toBeTruthy();
  await button!.trigger("click");
  if (waitForFlush) {
    await flushPromises();
  }
}

function createConfigStatus(overrides: Partial<Aria2ConfigStatus> = {}): Aria2ConfigStatus {
  return {
    configured: true,
    path: null,
    pathExists: true,
    binarySource: "sidecar",
    sidecarName: "aria2-next",
    targetTriple: "x86_64-unknown-linux-gnu",
    rpcHost: "127.0.0.1",
    rpcPort: 6800,
    rpcSecretConfigured: true,
    ...overrides,
  };
}

function createProcessStatus(overrides: Partial<Aria2ProcessStatus> = {}): Aria2ProcessStatus {
  return {
    running: false,
    pid: null,
    binarySource: "sidecar",
    message: "未启动",
    ...overrides,
  };
}

function createRpcStatus(overrides: Partial<Aria2RpcStatus> = {}): Aria2RpcStatus {
  return {
    connected: false,
    version: null,
    message: "未连接",
    ...overrides,
  };
}

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((promiseResolve, promiseReject) => {
    resolve = promiseResolve;
    reject = promiseReject;
  });

  return { promise, resolve, reject };
}
