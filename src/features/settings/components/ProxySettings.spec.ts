import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const messageMocks = vi.hoisted(() => ({ success: vi.fn(), error: vi.fn() }));

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return { ...actual, useMessage: () => messageMocks };
});

vi.mock("../../../components/ui/AppDialog.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "AppDialogProxySettingsStub",
      props: {
        show: { type: Boolean, default: false },
        title: { type: String, default: "" },
        closeDisabled: { type: Boolean, default: false },
      },
      emits: ["update:show"],
      setup(props, { slots, attrs }) {
        return () =>
          props.show
            ? h("section", { ...attrs, "data-test": "app-dialog" }, [
                h("h2", props.title),
                slots.default?.(),
                slots.footer?.(),
              ])
            : null;
      },
    }),
  };
});

vi.mock("../../../components/ui/AppDialogActions.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "AppDialogActionsProxySettingsStub",
      setup(_, { slots }) {
        return () => h("div", { "data-test": "app-dialog-actions" }, slots.default?.());
      },
    }),
  };
});

vi.mock("../services/downloadProxyService", () => ({
  deleteDownloadProxy: vi.fn(),
  getDownloadProxyStatus: vi.fn(),
  updateDownloadProxy: vi.fn(),
}));

import {
  deleteDownloadProxy,
  getDownloadProxyStatus,
  updateDownloadProxy,
} from "../services/downloadProxyService";
import { useDownloadProxyStore } from "../stores/downloadProxyStore";
import type { DownloadProxyStatus } from "../../../types/settings";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import ProxySettings from "./ProxySettings.vue";
import { setLanguage } from "../../../i18n";

const mockedDelete = vi.mocked(deleteDownloadProxy);
const mockedGetStatus = vi.mocked(getDownloadProxyStatus);
const mockedUpdate = vi.mocked(updateDownloadProxy);

describe("ProxySettings", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("shows only the masked server status in a controlled password input", async () => {
    mockedGetStatus.mockResolvedValueOnce(configuredStatus());
    const { wrapper } = mountSettings();
    await flushPromises();

    expect(wrapper.text()).toContain("http://***:***@proxy.example.com:7890/");
    expect(wrapper.text()).not.toContain("original-password");
    const input = wrapper.get('[data-test="download-proxy-input"]').get("input");
    expect(input.attributes("type")).toBe("password");
    expect((input.element as HTMLInputElement).value).toBe("");
  });

  it("confirms saving, clears the raw draft, and reports task application results", async () => {
    mockedGetStatus.mockResolvedValueOnce({
      configured: false,
      maskedProxyUrl: null,
      revision: 0,
      btUseProxy: false,
      btRuntimeApply: "deferred",
    });
    mockedUpdate.mockResolvedValueOnce({
      status: configuredStatus(),
      appliedTaskIds: [1],
      deferredTaskIds: [2],
      failed: [{ taskId: 3, code: "runtime_transition", message: "Aria2 正在切换运行状态，请稍后重试" }],
    });
    const { wrapper } = mountSettings();
    await flushPromises();
    const rawProxy = "http://user:original-password@proxy.example.com:7890";

    await wrapper.get('[data-test="download-proxy-input"]').get("input").setValue(rawProxy);
    await findButton(wrapper, "保存代理").trigger("click");

    expect(wrapper.text()).toContain("确认保存下载代理");
    await findButton(wrapper, "保存代理", true).trigger("click");
    await flushPromises();

    const store = useDownloadProxyStore();
    expect(mockedUpdate).toHaveBeenCalledWith(rawProxy);
    expect(store.draftProxyUrl).toBe("");
    expect(wrapper.text()).not.toContain("original-password");
    expect(wrapper.get('[data-test="download-proxy-result"]').text()).toContain("已应用 1 个，延后 1 个，失败 1 个");
    expect(wrapper.text()).toContain("#3 · Aria2 正在切换运行状态，请稍后重试");
    expect(messageMocks.success).toHaveBeenCalledWith("下载代理已保存");
    try {
      setLanguage("en-US");
      await wrapper.vm.$nextTick();
      expect(wrapper.get(".proxy-settings-failures").text()).toContain("The engine is changing state");
      expect(store.lastMutation?.failed[0].message).toBe("Aria2 正在切换运行状态，请稍后重试");
    } finally {
      setLanguage("zh-CN");
      wrapper.unmount();
    }
  });

  it("uses a replacement confirmation and clears the draft when settings close", async () => {
    mockedGetStatus.mockResolvedValueOnce(configuredStatus());
    const { wrapper } = mountSettings();
    await flushPromises();

    await wrapper.get('[data-test="download-proxy-input"]').get("input").setValue("socks5://proxy.example.com:1080");
    await findButton(wrapper, "替换代理").trigger("click");
    expect(wrapper.text()).toContain("确认替换下载代理");

    await wrapper.setProps({ active: false });

    expect(useDownloadProxyStore().draftProxyUrl).toBe("");
    expect(wrapper.find('[data-test="app-dialog"]').exists()).toBe(false);
  });

  it("clears configured proxy through a separate confirmation", async () => {
    mockedGetStatus.mockResolvedValueOnce(configuredStatus());
    mockedDelete.mockResolvedValueOnce(undefined);
    const { wrapper } = mountSettings();
    await flushPromises();

    await findButton(wrapper, "清除代理").trigger("click");
    expect(wrapper.text()).toContain("确认清除下载代理");
    await findButton(wrapper, "清除代理", true).trigger("click");
    await flushPromises();

    expect(mockedDelete).toHaveBeenCalledOnce();
    expect(wrapper.get('[data-test="download-proxy-status"]').text()).toContain("未配置");
    expect(messageMocks.success).toHaveBeenCalledWith("下载代理已清除");
  });

  it("shows the BT application state and updates the global switch", async () => {
    mockedGetStatus.mockResolvedValueOnce({
      ...configuredStatus(),
      btUseProxy: true,
      btRuntimeApply: "applied",
    });
    mockedUpdate.mockResolvedValueOnce({
      status: { ...configuredStatus(), btUseProxy: false, btRuntimeApply: "deferred" },
      appliedTaskIds: [],
      deferredTaskIds: [],
      failed: [],
    });
    const { wrapper } = mountSettings();
    await flushPromises();

    expect(wrapper.get('[data-test="bt-proxy-runtime-status"]').text()).toContain("BT 代理已应用");
    const toggle = wrapper.get('[data-test="bt-proxy-switch"]');
    expect(toggle.attributes("aria-checked")).toBe("true");
    await toggle.trigger("click");
    await flushPromises();

    expect(mockedUpdate).toHaveBeenCalledWith(undefined, false);
    expect(toggle.attributes("aria-checked")).toBe("false");
    expect(wrapper.find('[data-test="bt-proxy-runtime-status"]').exists()).toBe(false);
  });

  it.each([
    { name: "unconfigured", configured: false, enabled: false, apply: "deferred", expected: null },
    { name: "disabled", configured: true, enabled: false, apply: "deferred", expected: null },
    { name: "enabled and deferred", configured: true, enabled: true, apply: "deferred", expected: "将在引擎启动后应用" },
    { name: "blocked while disabling", configured: true, enabled: false, apply: "blocked", expected: "应用失败，BT 任务保持暂停" },
  ] as const)("shows the relevant BT runtime status when $name", async ({ configured, enabled, apply, expected }) => {
    mockedGetStatus.mockResolvedValueOnce({
      ...configuredStatus(),
      configured,
      maskedProxyUrl: configured ? configuredStatus().maskedProxyUrl : null,
      btUseProxy: enabled,
      btRuntimeApply: apply,
    });
    const { wrapper } = mountSettings();
    await flushPromises();

    const status = wrapper.find('[data-test="bt-proxy-runtime-status"]');
    expect(status.exists()).toBe(expected !== null);
    if (expected) expect(status.text()).toContain(expected);
  });
});

function mountSettings() {
  const pinia = createPinia();
  setActivePinia(pinia);
  return mountWithPinia(ProxySettings, {
    pinia,
    props: { active: true },
    global: { stubs: { teleport: true } },
  });
}

function configuredStatus(): DownloadProxyStatus {
  return {
    configured: true,
    maskedProxyUrl: "http://***:***@proxy.example.com:7890/",
    revision: 4,
    btUseProxy: false,
    btRuntimeApply: "deferred",
  };
}

function findButton(
  wrapper: ReturnType<typeof mountSettings>["wrapper"],
  text: string,
  last = false,
) {
  const buttons = wrapper.findAll("button").filter((button) => button.text().includes(text));
  return buttons[last ? buttons.length - 1 : 0]!;
}
