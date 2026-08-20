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
import { flushPromises, mountWithPinia } from "../../../test/mount";
import ProxySettings from "./ProxySettings.vue";

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
    mockedGetStatus.mockResolvedValueOnce({ configured: false, maskedProxyUrl: null, revision: 0 });
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

function configuredStatus() {
  return {
    configured: true,
    maskedProxyUrl: "http://***:***@proxy.example.com:7890/",
    revision: 4,
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
