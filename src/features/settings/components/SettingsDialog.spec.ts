import { ref } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";

const isMobileLayout = ref(false);
const tabsContextKey = Symbol("settings-tabs");

const message = {
  success: vi.fn(),
  error: vi.fn(),
  warning: vi.fn(),
  info: vi.fn(),
};

vi.mock("naive-ui", async () => {
  const { computed, defineComponent, h, inject, provide, ref: vueRef, toRef } = await import("vue");
  const slotStub = (name: string) =>
    defineComponent({
      name,
      setup(_, { slots }) {
        return () => h("div", { "data-test": name }, slots.default?.());
      },
    });

  const NTabPane = defineComponent({
    name: "NTabPaneStub",
    props: {
      name: { type: String, required: true },
      tab: { type: String, default: "" },
      displayDirective: { type: String, default: "if" },
    },
    setup(props, { slots }) {
      const context = inject<any>(tabsContextKey);
      if (!context) throw new Error("NTabPane must be nested in NTabs");
      const isActive = computed(() => context.value.value === props.name);
      const shouldRender = computed(() => isActive.value || context.rendered.value.has(props.name));

      return () => {
        if (isActive.value) {
          context.rendered.value.add(props.name);
        }
        if (!shouldRender.value) return null;
        return h(
          "div",
          {
            "data-test": "n-tab-pane",
            "data-pane": props.name,
            style: { display: isActive.value ? "" : "none" },
          },
          slots.default?.(),
        );
      };
    },
  });

  const NTabs = defineComponent({
    name: "NTabsStub",
    inheritAttrs: false,
    props: {
      value: { type: String, default: "" },
    },
    emits: ["update:value"],
    setup(props, { attrs, emit, slots }) {
      const rendered = vueRef(new Set<string>());
      const value = toRef(props, "value");
      provide(tabsContextKey, { value, rendered });

      return () => {
        const panes = (slots.default?.() ?? []).filter((vnode) => typeof vnode.type === "object");
        return h("div", { ...attrs, "data-test": "n-tabs" }, [
          h(
            "div",
            { "data-test": "n-tabs-nav" },
            panes.map((pane) =>
              h(
                "button",
                {
                  type: "button",
                  "data-tab": pane.props?.name,
                  "aria-selected": String(pane.props?.name === props.value),
                  onClick: () => emit("update:value", pane.props?.name),
                },
                pane.props?.tab,
              ),
            ),
          ),
          h("div", { "data-test": "n-tabs-content" }, slots.default?.()),
        ]);
      };
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
    NSelect: defineComponent({
      name: "NSelectStub",
      inheritAttrs: false,
      props: {
        value: { type: [String, Number], default: "" },
        options: { type: Array, default: () => [] },
      },
      emits: ["update:value"],
      setup(props, { attrs, emit }) {
        return () =>
          h(
            "select",
            {
              ...attrs,
              value: props.value,
              onChange: (event: Event) => emit("update:value", (event.target as HTMLSelectElement).value),
            },
            (props.options as Array<{ label: string; value: string | number }>).map((option) =>
              h("option", { value: option.value }, option.label),
            ),
          );
      },
    }),
    NTabPane,
    NTabs,
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
        fixedBody: Boolean,
        contentClass: String,
      },
      emits: ["update:show"],
      setup(props, { emit, slots }) {
        return () =>
          props.show
            ? h("section", { "data-test": "app-dialog", "data-fixed-body": String(props.fixedBody), "data-content-class": props.contentClass }, [
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
  },
}));

vi.mock("../../../app/composables/useMobileLayout", () => ({
  useMobileLayout: () => ({ isMobileLayout }),
}));

vi.mock("../../auth/components/WebAuthSettings.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "WebAuthSettingsStub",
      setup: () => () => h("div", { "data-test": "web-auth-settings" }, "Web 管理安全"),
    }),
  };
});

vi.mock("./JsonRpcTokenSettings.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "JsonRpcTokenSettingsStub",
      props: { active: Boolean },
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
      props: { active: Boolean },
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
      props: { active: Boolean },
      setup: () => () => h("div", { "data-test": "download-proxy-settings" }, "下载代理专用设置"),
    }),
  };
});

import SettingsDialog from "./SettingsDialog.vue";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import { fnosHost } from "../../../services/fnos";
import { refreshAccessiblePaths } from "../../../services/storage";
import { saveAppConfig } from "../../../services/settings";

describe("SettingsDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isMobileLayout.value = false;
    vi.mocked(fnosHost.getHostKind).mockResolvedValue("hosted");
    vi.mocked(fnosHost.requestSharedFolderAuthorization).mockResolvedValue({ status: "authorized" });
    vi.mocked(refreshAccessiblePaths).mockResolvedValue({ paths: ["/downloads"] });
  });

  it("renders the default section with fixed dialog content and footer actions", async () => {
    const { wrapper } = mountWithPinia(SettingsDialog, {
      props: {
        show: true,
      },
    });
    await flushPromises();

    expect(wrapper.text()).toContain("设置");
    expect(wrapper.text()).toContain("默认下载目录");
    expect(wrapper.get('[data-test="app-dialog"]').attributes("data-fixed-body")).toBe("true");
    expect(wrapper.get('[data-test="app-dialog"]').attributes("data-content-class")).toBe("settings-dialog-content");
    expect(wrapper.get('[data-pane="preferences"]').isVisible()).toBe(true);
    expect(wrapper.find('[data-test="download-proxy-settings"]').exists()).toBe(false);
    expect(wrapper.text()).toContain("保存");
    expect(wrapper.get('[data-test="app-dialog-actions"]').text()).toContain("保存");
  });

  it("switches all settings sections and lazily mounts each child", async () => {
    const { wrapper } = mountWithPinia(SettingsDialog, { props: { show: true } });
    await flushPromises();

    await selectMainSection(wrapper, "下载代理");
    expect(wrapper.find('[data-test="download-proxy-settings"]').exists()).toBe(true);
    await selectMainSection(wrapper, "Web 管理安全");
    expect(wrapper.find('[data-test="web-auth-settings"]').exists()).toBe(true);
    await selectMainSection(wrapper, "RPC 访问");
    expect(wrapper.find('[data-test="open-rpc-guide"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="open-lan-rpc-guide"]').exists()).toBe(false);

    await selectRpcSection(wrapper, "局域网入口");
    expect(wrapper.find('[data-test="open-lan-rpc-guide"]').exists()).toBe(true);
    await selectMainSection(wrapper, "下载代理");
    await selectMainSection(wrapper, "RPC 访问");
    expect(wrapper.find('[data-test="open-lan-rpc-guide"]').exists()).toBe(true);
  });

  it("uses the mobile section select to control the same panes", async () => {
    isMobileLayout.value = true;
    const { wrapper } = mountWithPinia(SettingsDialog, { props: { show: true } });
    await flushPromises();

    const select = wrapper.get(".settings-section-select");
    await select.setValue("rpc");
    await flushPromises();

    expect((select.element as HTMLSelectElement).value).toBe("rpc");
    expect(wrapper.find('[data-test="open-rpc-guide"]').exists()).toBe(true);
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

    await selectMainSection(wrapper, "RPC 访问");
    await wrapper.get('[data-test="open-rpc-guide"]').trigger("click");
    await selectRpcSection(wrapper, "局域网入口");
    await wrapper.get('[data-test="open-lan-rpc-guide"]').trigger("click");

    expect(wrapper.emitted("openRpcGuide")).toHaveLength(2);
  });

  it("keeps the footer save action scoped to the regular configuration", async () => {
    const { wrapper } = mountWithPinia(SettingsDialog, { props: { show: true } });
    await flushPromises();

    await wrapper.findAll("button").find((button) => button.text() === "保存")!.trigger("click");
    await flushPromises();

    expect(saveAppConfig).toHaveBeenCalledOnce();
    expect(wrapper.emitted("update:show")).toContainEqual([false]);
  });

  it("resets navigation to the regular section when reopened", async () => {
    const { wrapper } = mountWithPinia(SettingsDialog, { props: { show: true } });
    await flushPromises();
    await selectMainSection(wrapper, "RPC 访问");
    expect(wrapper.find('[data-test="open-rpc-guide"]').exists()).toBe(true);

    await wrapper.setProps({ show: false });
    await wrapper.setProps({ show: true });
    await flushPromises();

    expect(wrapper.find('[data-pane="preferences"]').isVisible()).toBe(true);
    expect(wrapper.find('[data-test="open-rpc-guide"]').exists()).toBe(false);
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

    expect(wrapper.text()).toContain("当前页面不是支持的 fnOS 宿主");
    expect(wrapper.findAll("button").some((button) => button.text() === "添加授权文件夹")).toBe(false);
    expect(fnosHost.requestSharedFolderAuthorization).not.toHaveBeenCalled();
  });
});

async function selectMainSection(wrapper: any, label: string) {
  await wrapper.get(".settings-sections-tabs").findAll("button").find((button: any) => button.text() === label)!.trigger("click");
  await flushPromises();
}

async function selectRpcSection(wrapper: any, label: string) {
  await wrapper.get(".settings-rpc-tabs").findAll("button").find((button: any) => button.text() === label)!.trigger("click");
  await flushPromises();
}
