import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import type { Aria2LogModeStatus } from "../types";

const messageApi = vi.hoisted(() => ({ success: vi.fn() }));
const logModeService = vi.hoisted(() => ({
  getAria2LogMode: vi.fn(),
  updateAria2LogMode: vi.fn(),
}));

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return { ...actual, useMessage: () => messageApi };
});

vi.mock("../services/aria2LogModeService", () => logModeService);

import Aria2LogModePanel from "./Aria2LogModePanel.vue";

describe("Aria2LogModePanel", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-11T08:00:00Z"));
    vi.clearAllMocks();
    logModeService.getAria2LogMode.mockResolvedValue(normalStatus());
  });

  afterEach(() => vi.useRealTimers());

  it("loads the active mode and explains stopped-engine detailed logging", async () => {
    logModeService.getAria2LogMode.mockResolvedValueOnce(detailedStatus({ appliesOnNextStart: true }));
    const { wrapper } = mountPanel();

    await flushPromises();

    expect(logModeService.getAria2LogMode).toHaveBeenCalledOnce();
    expect(wrapper.text()).toContain("详细（debug）");
    expect(wrapper.text()).toContain("剩余 30:00");
    expect(wrapper.text()).toContain("单文件 12 MiB，最多 3 个文件");
    expect(wrapper.text()).toContain("Aria2 当前未运行，详细日志将在下一次受控启动时生效。");
    expect(findButton(wrapper, "开启详细日志（30 分钟）").attributes("disabled")).toBeDefined();
    expect(findButton(wrapper, "恢复普通日志").attributes("disabled")).toBeUndefined();
  });

  it("enables detailed logging and emits the updated mode", async () => {
    const nextStatus = detailedStatus();
    logModeService.updateAria2LogMode.mockResolvedValueOnce(nextStatus);
    const { wrapper } = mountPanel();
    await flushPromises();

    await findButton(wrapper, "开启详细日志（30 分钟）").trigger("click");
    await flushPromises();

    expect(logModeService.updateAria2LogMode).toHaveBeenCalledWith(true);
    expect(wrapper.emitted("updated")).toEqual([[nextStatus]]);
    expect(messageApi.success).toHaveBeenCalledWith("已开启详细日志，30 分钟后自动恢复");
    expect(findButton(wrapper, "恢复普通日志").attributes("disabled")).toBeUndefined();
  });

  it("restores normal logging and keeps the error visible when a change fails", async () => {
    logModeService.getAria2LogMode.mockResolvedValueOnce(detailedStatus());
    logModeService.updateAria2LogMode
      .mockRejectedValueOnce(new Error("切换失败"))
      .mockResolvedValueOnce(normalStatus());
    const { wrapper } = mountPanel();
    await flushPromises();

    await findButton(wrapper, "恢复普通日志").trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("切换失败");
    expect(wrapper.emitted("updated")).toBeUndefined();

    await findButton(wrapper, "恢复普通日志").trigger("click");
    await flushPromises();

    expect(logModeService.updateAria2LogMode).toHaveBeenNthCalledWith(1, false);
    expect(logModeService.updateAria2LogMode).toHaveBeenNthCalledWith(2, false);
    expect(wrapper.emitted("updated")).toEqual([[normalStatus()]]);
    expect(messageApi.success).toHaveBeenCalledWith("已恢复普通日志");
  });

  it("does not load the mode until the diagnostics dialog becomes active", async () => {
    const { wrapper } = mountPanel(false);
    await flushPromises();

    expect(logModeService.getAria2LogMode).not.toHaveBeenCalled();

    await wrapper.setProps({ active: true });
    await flushPromises();

    expect(logModeService.getAria2LogMode).toHaveBeenCalledOnce();
  });
});

function mountPanel(active = true) {
  return mountWithPinia(Aria2LogModePanel, { props: { active } });
}

function normalStatus(): Aria2LogModeStatus {
  return {
    mode: "warn",
    detailed: false,
    detailedUntilMs: null,
    maxFileSizeBytes: 10 * 1024 * 1024,
    maxFileCount: 3,
    appliesOnNextStart: false,
  };
}

function detailedStatus(overrides: Partial<Aria2LogModeStatus> = {}): Aria2LogModeStatus {
  return {
    mode: "debug",
    detailed: true,
    detailedUntilMs: Date.now() + 30 * 60 * 1000,
    maxFileSizeBytes: 12 * 1024 * 1024,
    maxFileCount: 3,
    appliesOnNextStart: false,
    ...overrides,
  };
}

function findButton(wrapper: ReturnType<typeof mountPanel>["wrapper"], text: string) {
  const button = wrapper.findAll("button").find((item) => item.text().includes(text));
  if (!button) {
    throw new Error(`button not found: ${text}`);
  }
  return button;
}
