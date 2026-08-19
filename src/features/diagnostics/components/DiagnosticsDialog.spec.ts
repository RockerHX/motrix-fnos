import { beforeEach, describe, expect, it, vi } from "vitest";

const diagnosticBundleExport = vi.hoisted(() => ({ exportDiagnosticBundle: vi.fn() }));

vi.mock("naive-ui", async () => {
  const { computed, defineComponent, h, inject, provide, ref, toRef } = await import("vue");
  const tabsContextKey = Symbol("diagnostics-tabs");
  const NTabPaneStub = defineComponent({
    name: "NTabPaneStub",
    props: {
      name: { type: String, required: true },
      tab: { type: String, default: "" },
    },
    setup(props, { slots }) {
      const context = inject<any>(tabsContextKey);
      if (!context) throw new Error("NTabPane must be nested in NTabs");
      const isActive = computed(() => context.value.value === props.name);
      const shouldRender = computed(() => isActive.value || context.rendered.value.has(props.name));

      return () => {
        if (isActive.value) context.rendered.value.add(props.name);
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
  const NTabsStub = defineComponent({
    name: "NTabsStub",
    inheritAttrs: false,
    props: {
      value: { type: String, default: "" },
      type: { type: String, default: "bar" },
    },
    emits: ["update:value"],
    setup(props, { attrs, emit, slots }) {
      const rendered = ref(new Set<string>());
      const value = toRef(props, "value");
      provide(tabsContextKey, { value, rendered });

      return () => {
        const panes = (slots.default?.() ?? []).filter((vnode) => typeof vnode.type === "object");
        return h("div", { ...attrs, "data-test": "n-tabs", "data-tabs-type": props.type }, [
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
    NButton: defineComponent({
      name: "NButtonStub",
      props: {
        disabled: { type: Boolean, default: false },
        loading: { type: Boolean, default: false },
      },
      emits: ["click"],
      setup(props, { attrs, emit, slots }) {
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
    NSpace: defineComponent({
      name: "NSpaceStub",
      setup(_, { slots }) {
        return () => h("div", { "data-test": "n-space" }, slots.default?.());
      },
    }),
    NTabPane: NTabPaneStub,
    NTabs: NTabsStub,
  };
});

vi.mock("../../settings/services/lanJsonRpcService", () => ({
  getLanJsonRpcStatus: vi.fn(async () => ({ enabled: true, configured: true, maskedToken: "••••••••1234", port: 17082 })),
  rotateLanJsonRpcToken: vi.fn(),
  updateLanJsonRpcEnabled: vi.fn(),
}));

vi.mock("../../../components/ui/AppDialog.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "AppDialogStub",
      props: {
        show: Boolean,
        title: String,
        eyebrow: String,
        fixedBody: Boolean,
        contentClass: String,
      },
      emits: ["update:show"],
      setup(props, { emit, slots }) {
        return () =>
          props.show
            ? h("section", { "data-test": "app-dialog", "data-fixed-body": String(props.fixedBody), "data-content-class": props.contentClass }, [
                h("p", props.eyebrow),
                h("h2", props.title),
                slots["header-extra"]?.(),
                slots.default?.(),
                h("button", { "aria-label": "关闭", onClick: () => emit("update:show", false) }, "×"),
              ])
            : null;
      },
    }),
  };
});

vi.mock("../composables/useDiagnosticBundleExport", async () => {
  const { ref } = await import("vue");
  return {
    useDiagnosticBundleExport: () => ({
      isExporting: ref(false),
      exportDiagnosticBundle: diagnosticBundleExport.exportDiagnosticBundle,
    }),
  };
});

import DiagnosticsDialog from "./DiagnosticsDialog.vue";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import type { AppInfo, BackendPing } from "../../../types/app";

describe("DiagnosticsDialog", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders the overview by default with fixed dialog content", async () => {
    const { wrapper } = mountDialog(false);

    await wrapper.setProps({ show: true });
    await flushPromises();

    expect(wrapper.text()).toContain("诊断");
    expect(wrapper.text()).toContain("1.6.1");
    expect(wrapper.text()).toContain("pong");
    expect(wrapper.get('[data-test="app-dialog"]').attributes("data-fixed-body")).toBe("true");
    expect(wrapper.get('[data-test="app-dialog"]').attributes("data-content-class")).toBe("diagnostics-dialog-content");
    expect(wrapper.get(".diagnostics-tabs").attributes("data-tabs-type")).toBe("line");
    expect(wrapper.get('[data-pane="overview"]').isVisible()).toBe(true);
    expect(wrapper.find('[data-pane="connection"]').exists()).toBe(false);
    expect(wrapper.emitted("refreshStatus")).toHaveLength(1);
  });

  it("switches connection and logs panes without repeating status refresh", async () => {
    const { wrapper } = mountDialog(false);

    await wrapper.setProps({ show: true });
    await flushPromises();
    await selectSection(wrapper, "连接");

    expect(wrapper.text()).toContain("127.0.0.1:17081");
    expect(wrapper.text()).toContain("17082/jsonrpc");
    expect(wrapper.text()).toContain("局域网入口 / Token");
    expect(wrapper.text()).toContain("已配置");
    expect(wrapper.find('[data-test="aria2-log-mode-updated"]').exists()).toBe(false);
    expect(wrapper.emitted("refreshStatus")).toHaveLength(1);

    await selectSection(wrapper, "日志");
    expect(wrapper.find('[data-test="aria2-log-mode-updated"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="log-maintenance-stub"]').exists()).toBe(true);
    expect(wrapper.emitted("refreshStatus")).toHaveLength(1);
  });

  it("refreshes diagnostics status after the log mode changes", async () => {
    const { wrapper } = mountDialog(false);

    await wrapper.setProps({ show: true });
    await flushPromises();
    await selectSection(wrapper, "日志");
    await wrapper.get('[data-test="aria2-log-mode-updated"]').trigger("click");

    expect(wrapper.emitted("refreshStatus")).toHaveLength(2);
  });

  it("opens debug logs and emits close event", async () => {
    const { wrapper } = mountDialog();

    await selectSection(wrapper, "日志");
    await wrapper.findAll("button").find((button) => button.text() === "调试日志")!.trigger("click");
    await wrapper.get('button[aria-label="关闭"]').trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("debug-log-stub");
    expect(wrapper.emitted("update:show")).toEqual([[false]]);
  });

  it("exports a diagnostic bundle through the authenticated client", async () => {
    const { wrapper } = mountDialog();

    await wrapper.findAll("button").find((button) => button.text() === "导出诊断包")!.trigger("click");

    expect(diagnosticBundleExport.exportDiagnosticBundle).toHaveBeenCalledOnce();
  });

  it("opens the independent RPC guide from diagnostics", async () => {
    const { wrapper } = mountDialog();

    await selectSection(wrapper, "连接");
    await wrapper.findAll("button").find((button) => button.text() === "JSON-RPC 指南")!.trigger("click");

    expect(wrapper.emitted("openRpcGuide")).toHaveLength(1);
  });

  it("forwards engine status updates from the overview pane", async () => {
    const { wrapper } = mountDialog();

    await wrapper.get('[data-test="engine-status-updated"]').trigger("click");

    expect(wrapper.emitted("engineStatusUpdated")).toHaveLength(1);
  });
});

function mountDialog(show = true) {
  return mountWithPinia(DiagnosticsDialog, {
    props: {
      show,
      appInfo: createAppInfo(),
      backendPing: createBackendPing(),
      aria2Process: { running: true, pid: 1, binarySource: "sidecar", message: "running" },
      aria2Rpc: { connected: true, version: "1.37.0", message: "ok" },
      jsonRpcTokenConfigured: true,
    },
    global: {
      stubs: {
        Aria2LogModePanel: {
          name: "Aria2LogModePanelStub",
          props: ["active"],
          emits: ["updated"],
          template: "<button data-test='aria2-log-mode-updated' @click='$emit(\"updated\")'>aria2-log-mode-stub</button>",
        },
        EngineStatusPanel: {
          template: "<button data-test='engine-status-updated' @click='$emit(\"status-updated\", { process: { running: true }, rpc: { connected: true } })'>engine-status-stub</button>",
          emits: ["status-updated"],
        },
        LogMaintenancePanel: {
          props: ["active", "aria2Running"],
          template: "<div data-test='log-maintenance-stub'>log-maintenance-stub</div>",
        },
        DebugLogDialog: { props: ["show"], template: "<div v-if='show'>debug-log-stub</div>" },
      },
    },
  });
}

async function selectSection(wrapper: any, label: string) {
  await wrapper.get(".diagnostics-tabs").findAll("button").find((button: any) => button.text() === label)!.trigger("click");
  await flushPromises();
}

function createAppInfo(): AppInfo {
  return {
    name: "Motrix fnOS",
    version: "1.6.1",
    backendStatus: "ok",
    updateMode: "manual_fpk_or_app_center",
    maintainer: "tester",
    repositoryUrl: "https://example.com/repo",
    releasePageUrl: "https://example.com/releases",
    targetArch: "x86_64",
  };
}

function createBackendPing(): BackendPing {
  return {
    ok: true,
    message: "pong",
  };
}
