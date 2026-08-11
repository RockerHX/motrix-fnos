import { describe, expect, it, vi } from "vitest";

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
      },
      emits: ["update:show"],
      setup(props, { emit, slots }) {
        return () =>
          props.show
            ? h("section", { "data-test": "app-dialog" }, [
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

import DiagnosticsDialog from "./DiagnosticsDialog.vue";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import type { AppInfo, BackendPing } from "../../../types/app";

describe("DiagnosticsDialog", () => {
  it("renders diagnostics status and emits refresh when opened", async () => {
    const { wrapper } = mountDialog(false);

    await wrapper.setProps({ show: true });
    await flushPromises();

    expect(wrapper.text()).toContain("诊断");
    expect(wrapper.text()).toContain("1.6.1");
    expect(wrapper.text()).toContain("pong");
    expect(wrapper.text()).toContain("127.0.0.1:17081");
    expect(wrapper.text()).toContain("17082/jsonrpc");
    expect(wrapper.text()).toContain("局域网入口 / Token");
    expect(wrapper.text()).toContain("已配置");
    expect(wrapper.emitted("refreshStatus")).toHaveLength(1);
  });

  it("refreshes diagnostics status after the log mode changes", async () => {
    const { wrapper } = mountDialog(false);

    await wrapper.setProps({ show: true });
    await flushPromises();
    await wrapper.get('[data-test="aria2-log-mode-updated"]').trigger("click");

    expect(wrapper.emitted("refreshStatus")).toHaveLength(2);
  });

  it("opens debug logs and emits close event", async () => {
    const { wrapper } = mountDialog();

    await wrapper.findAll("button").find((button) => button.text() === "调试日志")!.trigger("click");
    await wrapper.get('button[aria-label="关闭"]').trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("debug-log-stub");
    expect(wrapper.emitted("update:show")).toEqual([[false]]);
  });

  it("opens the independent RPC guide from diagnostics", async () => {
    const { wrapper } = mountDialog();

    await wrapper.findAll("button").find((button) => button.text() === "JSON-RPC 指南")!.trigger("click");

    expect(wrapper.emitted("openRpcGuide")).toHaveLength(1);
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
        EngineStatusPanel: { template: "<div>engine-status-stub</div>" },
        DebugLogDialog: { props: ["show"], template: "<div v-if='show'>debug-log-stub</div>" },
      },
    },
  });
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
