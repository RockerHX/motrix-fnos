import { beforeEach, describe, expect, it, vi } from "vitest";

const message = {
  success: vi.fn(),
  error: vi.fn(),
  warning: vi.fn(),
  info: vi.fn(),
};

vi.mock("naive-ui", async () => {
  const { defineComponent, h } = await import("vue");
  const slotStub = (name: string) =>
    defineComponent({
      name,
      setup(_, { slots }) {
        return () => h("div", { "data-test": name }, slots.default?.());
      },
    });

  return {
    NAlert: slotStub("n-alert"),
    NButton: defineComponent({
      name: "NButtonStub",
      props: {
        disabled: { type: Boolean, default: false },
        loading: { type: Boolean, default: false },
      },
      emits: ["click"],
      setup(props, { emit, slots, attrs }) {
        return () =>
          h(
            "button",
            {
              ...attrs,
              disabled: props.disabled,
              onClick: (event: MouseEvent) => {
                if (!props.disabled) emit("click", event);
              },
            },
            slots.default?.(),
          );
      },
    }),
    NForm: slotStub("n-form"),
    NFormItem: defineComponent({
      name: "NFormItemStub",
      props: {
        label: {
          type: String,
          default: "",
        },
      },
      setup(props, { slots }) {
        return () => h("div", { "data-test": "n-form-item" }, [props.label, slots.default?.()]);
      },
    }),
    NInput: slotStub("n-input"),
    NInputNumber: slotStub("n-input-number"),
    NSelect: slotStub("n-select"),
    NSpace: slotStub("n-space"),
    NText: slotStub("n-text"),
    useMessage: () => message,
  };
});

vi.mock("../../../components/ui/AppDialog.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "AppDialogStub",
      props: {
        show: Boolean,
        title: String,
      },
      emits: ["update:show"],
      setup(props, { emit, slots }) {
        return () =>
          props.show
            ? h("section", { "data-test": "app-dialog" }, [
                h("h2", props.title),
                slots.default?.(),
                slots.footer?.(),
                h("button", { "aria-label": "关闭", onClick: () => emit("update:show", false) }, "×"),
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
      name: "AppDialogActionsStub",
      setup(_, { slots }) {
        return () => h("div", { "data-test": "app-dialog-actions" }, slots.default?.());
      },
    }),
  };
});

vi.mock("../../../services/settings", () => ({
  getAppConfig: vi.fn(async () => ({
    defaultDownloadDir: "/downloads",
    maxConcurrentDownloads: 5,
    downloadLimit: 0,
    uploadLimit: 0,
    language: "zh-CN",
  })),
  saveAppConfig: vi.fn(async (payload) => payload),
}));

vi.mock("../../../services/storage", () => ({
  getAccessiblePaths: vi.fn(async () => ({ paths: ["/downloads"] })),
  refreshAccessiblePaths: vi.fn(async () => ({ paths: ["/downloads"] })),
}));

vi.mock("../../../services/fnos", () => ({
  fnosHost: {
    getHostKind: vi.fn(async () => "hosted"),
    requestSharedFolderAuthorization: vi.fn(async () => ({ status: "authorized" })),
    openAppSettings: vi.fn(async () => ({ status: "opened" })),
  },
}));

vi.mock("../../auth/components/WebAuthSettings.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return { default: defineComponent({ name: "WebAuthSettingsStub", setup: () => () => h("div", "Web 管理安全") }) };
});

vi.mock("./JsonRpcTokenSettings.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "JsonRpcTokenSettingsStub",
      emits: ["openGuide"],
      setup(_, { emit }) {
        return () =>
          h("div", [
            h("span", "JSON-RPC Token 专用设置"),
            h("button", { "data-test": "open-rpc-guide", onClick: () => emit("openGuide") }, "查看使用指南"),
          ]);
      },
    }),
  };
});

vi.mock("./LanJsonRpcSettings.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "LanJsonRpcSettingsStub",
      emits: ["openGuide"],
      setup(_, { emit }) {
        return () =>
          h("div", [
            h("span", "局域网推送设置"),
            h("button", { "data-test": "open-lan-rpc-guide", onClick: () => emit("openGuide") }, "查看局域网指南"),
          ]);
      },
    }),
  };
});

vi.mock("./ProxySettings.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "ProxySettingsStub",
      setup: () => () => h("div", "下载代理专用设置"),
    }),
  };
});

import SettingsDialog from "./SettingsDialog.vue";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import { fnosHost } from "../../../services/fnos";
import { refreshAccessiblePaths } from "../../../services/storage";

describe("SettingsDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(fnosHost.getHostKind).mockResolvedValue("hosted");
    vi.mocked(fnosHost.requestSharedFolderAuthorization).mockResolvedValue({ status: "authorized" });
    vi.mocked(fnosHost.openAppSettings).mockResolvedValue({ status: "opened" });
    vi.mocked(refreshAccessiblePaths).mockResolvedValue({ paths: ["/downloads"] });
  });

  it("renders settings form and footer actions", async () => {
    const { wrapper } = mountWithPinia(SettingsDialog, {
      props: {
        show: true,
      },
    });
    await flushPromises();

    expect(wrapper.text()).toContain("设置");
    expect(wrapper.text()).toContain("默认下载目录");
    expect(wrapper.text()).toContain("Web 管理安全");
    expect(wrapper.text()).toContain("下载代理专用设置");
    expect(wrapper.text()).toContain("JSON-RPC Token 专用设置");
    expect(wrapper.text()).toContain("保存");
    expect(wrapper.get('[data-test="app-dialog-actions"]').text()).toContain("保存");
  });

  it("emits close event from dialog and cancel button", async () => {
    const { wrapper } = mountWithPinia(SettingsDialog, {
      props: {
        show: true,
      },
    });
    await flushPromises();

    await wrapper.get('button[aria-label="关闭"]').trigger("click");
    await wrapper.findAll("button").find((button) => button.text() === "取消")!.trigger("click");
    await flushPromises();

    expect(wrapper.emitted("update:show")).toEqual([[false], [false]]);
  });

  it("forwards the RPC guide request from token settings", async () => {
    const { wrapper } = mountWithPinia(SettingsDialog, {
      props: {
        show: true,
      },
    });
    await flushPromises();

    await wrapper.get('[data-test="open-rpc-guide"]').trigger("click");

    expect(wrapper.emitted("openRpcGuide")).toHaveLength(1);
  });

  it("adds a folder through the host picker and refreshes from the backend", async () => {
    const { wrapper } = mountWithPinia(SettingsDialog, { props: { show: true } });
    await flushPromises();
    await flushPromises();

    await wrapper.findAll("button").find((button) => button.text() === "添加授权文件夹")!.trigger("click");
    await flushPromises();

    expect(fnosHost.requestSharedFolderAuthorization).toHaveBeenCalledOnce();
    expect(refreshAccessiblePaths).toHaveBeenCalledOnce();
    expect(message.success).toHaveBeenCalledWith("授权已更新，目录列表已刷新。");
  });

  it("does not refresh the backend when the picker is cancelled", async () => {
    vi.mocked(fnosHost.requestSharedFolderAuthorization).mockResolvedValueOnce({ status: "cancelled" });
    const { wrapper } = mountWithPinia(SettingsDialog, { props: { show: true } });
    await flushPromises();
    await flushPromises();

    await wrapper.findAll("button").find((button) => button.text() === "添加授权文件夹")!.trigger("click");
    await flushPromises();

    expect(refreshAccessiblePaths).not.toHaveBeenCalled();
    expect(message.error).not.toHaveBeenCalled();
  });

  it("shows manual authorization guidance outside fnOS host runtime", async () => {
    vi.mocked(fnosHost.getHostKind).mockResolvedValueOnce("standalone");
    const { wrapper } = mountWithPinia(SettingsDialog, { props: { show: true } });
    await flushPromises();
    await flushPromises();

    expect(wrapper.text()).toContain("当前环境不支持应用内选择");
    expect(wrapper.findAll("button").some((button) => button.text() === "添加授权文件夹")).toBe(false);
    expect(fnosHost.requestSharedFolderAuthorization).not.toHaveBeenCalled();
  });
});
