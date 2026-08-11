import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import type { DiagnosticsLogUsage } from "../types";

const messageApi = vi.hoisted(() => ({ success: vi.fn() }));
const logMaintenanceService = vi.hoisted(() => ({
  getLogUsage: vi.fn(),
  clearAria2Logs: vi.fn(),
}));

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  const { defineComponent, h } = await import("vue");
  return {
    ...actual,
    NPopconfirm: defineComponent({
      name: "NPopconfirmStub",
      props: { disabled: Boolean },
      emits: ["positive-click"],
      setup(props, { emit, slots }) {
        return () =>
          h("div", { "data-test": "n-popconfirm" }, [
            slots.trigger?.(),
            props.disabled
              ? null
              : h(
                  "button",
                  {
                    "data-test": "popconfirm-positive",
                    onClick: () => emit("positive-click"),
                  },
                  "confirm",
                ),
            slots.default?.(),
          ]);
      },
    }),
    useMessage: () => messageApi,
  };
});

vi.mock("../services/logMaintenanceService", () => logMaintenanceService);

import LogMaintenancePanel from "./LogMaintenancePanel.vue";

describe("LogMaintenancePanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    logMaintenanceService.getLogUsage.mockResolvedValue(createUsage());
    logMaintenanceService.clearAria2Logs.mockResolvedValue({
      reclaimedBytes: 30 * 1024 * 1024,
      usage: createUsage({ aria2: emptyUsage() }),
    });
  });

  it("loads and displays the total, current, historical, and file usage", async () => {
    const { wrapper } = mountPanel();
    await flushPromises();

    expect(logMaintenanceService.getLogUsage).toHaveBeenCalledOnce();
    expect(wrapper.text()).toContain("日志占用与维护");
    expect(wrapper.text()).toContain("68 MiB");
    expect(wrapper.text()).toContain("Aria2 原生日志");
    expect(wrapper.text()).toContain("当前 10 MiB，历史 20 MiB");
    expect(wrapper.text()).toContain("3 个文件");
  });

  it("warns when total usage reaches the warning threshold", async () => {
    logMaintenanceService.getLogUsage.mockResolvedValueOnce(createUsage({ totalBytes: 80 * 1024 * 1024 }));
    const { wrapper } = mountPanel();
    await flushPromises();

    expect(wrapper.text()).toContain("日志占用已达到预警阈值，请导出诊断包或清理已停止的 Aria2 日志。");
  });

  it("disables cleanup while Aria2 is running", async () => {
    const { wrapper } = mountPanel({ aria2Running: true });
    await flushPromises();

    expect(wrapper.text()).toContain("Aria2 正在运行。请先停止引擎，再清理 Aria2 原生日志。");
    expect(findButton(wrapper, "清理 Aria2 日志").attributes("disabled")).toBeDefined();
  });

  it("clears stopped-engine logs after confirmation and publishes the latest usage", async () => {
    const { wrapper } = mountPanel();
    await flushPromises();

    await wrapper.get('[data-test="popconfirm-positive"]').trigger("click");
    await flushPromises();

    expect(logMaintenanceService.clearAria2Logs).toHaveBeenCalledOnce();
    expect(wrapper.emitted("updated")).toEqual([[createUsage({ aria2: emptyUsage() })]]);
    expect(messageApi.success).toHaveBeenCalledWith("已清理 Aria2 日志，释放 30 MiB");
  });

  it("keeps cleanup failures visible and refreshes on demand", async () => {
    logMaintenanceService.clearAria2Logs.mockRejectedValueOnce(new Error("日志仍在使用"));
    const { wrapper } = mountPanel();
    await flushPromises();

    await wrapper.get('[data-test="popconfirm-positive"]').trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("日志仍在使用");

    await wrapper.get('button[aria-label="刷新日志占用"]').trigger("click");
    await flushPromises();
    expect(logMaintenanceService.getLogUsage).toHaveBeenCalledTimes(2);
  });

  it("does not load until its parent diagnostics dialog is active", async () => {
    const { wrapper } = mountPanel({ active: false });
    await flushPromises();
    expect(logMaintenanceService.getLogUsage).not.toHaveBeenCalled();

    await wrapper.setProps({ active: true });
    await flushPromises();
    expect(logMaintenanceService.getLogUsage).toHaveBeenCalledOnce();
  });
});

function mountPanel(props: Partial<{ active: boolean; aria2Running: boolean | null }> = {}) {
  return mountWithPinia(LogMaintenancePanel, {
    props: {
      active: true,
      aria2Running: false,
      ...props,
    },
  });
}

function createUsage(overrides: Partial<DiagnosticsLogUsage> = {}): DiagnosticsLogUsage {
  const aria2 = {
    currentBytes: 10 * 1024 * 1024,
    historyBytes: 20 * 1024 * 1024,
    totalBytes: 30 * 1024 * 1024,
    currentFileCount: 1,
    historyFileCount: 2,
    totalFileCount: 3,
  };
  const server = {
    currentBytes: 12 * 1024 * 1024,
    historyBytes: 14 * 1024 * 1024,
    totalBytes: 26 * 1024 * 1024,
    currentFileCount: 1,
    historyFileCount: 2,
    totalFileCount: 3,
  };
  const lifecycle = {
    currentBytes: 5 * 1024 * 1024,
    historyBytes: 7 * 1024 * 1024,
    totalBytes: 12 * 1024 * 1024,
    currentFileCount: 1,
    historyFileCount: 2,
    totalFileCount: 3,
  };
  return {
    aria2,
    server,
    lifecycle,
    totalBytes: aria2.totalBytes + server.totalBytes + lifecycle.totalBytes,
    totalFileCount: aria2.totalFileCount + server.totalFileCount + lifecycle.totalFileCount,
    aria2LogMode: {
      mode: "warn",
      detailed: false,
      detailedUntilMs: null,
      maxFileSizeBytes: 10 * 1024 * 1024,
      maxFileCount: 3,
      appliesOnNextStart: false,
    },
    ...overrides,
  };
}

function emptyUsage() {
  return {
    currentBytes: 0,
    historyBytes: 0,
    totalBytes: 0,
    currentFileCount: 0,
    historyFileCount: 0,
    totalFileCount: 0,
  };
}

function findButton(wrapper: ReturnType<typeof mountPanel>["wrapper"], text: string) {
  const button = wrapper.findAll("button").find((item) => item.text().includes(text));
  if (!button) {
    throw new Error(`button not found: ${text}`);
  }
  return button;
}
