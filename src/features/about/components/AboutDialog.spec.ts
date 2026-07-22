import { describe, expect, it, vi } from "vitest";

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
                slots.default?.(),
                h("button", { "aria-label": "关闭", onClick: () => emit("update:show", false) }, "×"),
              ])
            : null;
      },
    }),
  };
});

import AboutDialog from "./AboutDialog.vue";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import type { AppInfo } from "../../../types/app";

describe("AboutDialog", () => {
  it("renders app information and update action", () => {
    const { wrapper } = mountWithPinia(AboutDialog, {
      props: {
        show: true,
        appInfo: createAppInfo(),
        updateCheck: null,
      },
    });

    expect(wrapper.text()).toContain("About");
    expect(wrapper.text()).toContain("关于 Motrix fnOS");
    expect(wrapper.text()).toContain("v1.6.1");
    expect(wrapper.text()).toContain("检查更新");
  });

  it("emits close and checkUpdate events", async () => {
    const { wrapper } = mountWithPinia(AboutDialog, {
      props: {
        show: true,
        appInfo: createAppInfo(),
        updateCheck: null,
      },
    });

    await wrapper.get('button[aria-label="关闭"]').trigger("click");
    await wrapper.findAll("button").find((button) => button.text() === "检查更新")!.trigger("click");
    await flushPromises();

    expect(wrapper.emitted("update:show")).toEqual([[false]]);
    expect(wrapper.emitted("checkUpdate")).toHaveLength(1);
  });

  it("shows a compact entry for the independent RPC guide", async () => {
    const { wrapper } = mountWithPinia(AboutDialog, {
      props: {
        show: true,
        appInfo: createAppInfo(),
        updateCheck: null,
      },
    });

    expect(wrapper.text()).toContain("JSON-RPC 使用指南");
    await wrapper.findAll("button").find((button) => button.text() === "查看指南")!.trigger("click");

    expect(wrapper.emitted("openRpcGuide")).toHaveLength(1);
    expect(wrapper.emitted("update:show")).toContainEqual([false]);
  });
});

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
