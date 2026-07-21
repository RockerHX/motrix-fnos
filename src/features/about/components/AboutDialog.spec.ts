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

  it("explains the dedicated RPC port and opens settings from the guide", async () => {
    const { wrapper } = mountWithPinia(AboutDialog, {
      props: {
        show: true,
        appInfo: createAppInfo(),
        updateCheck: null,
      },
    });

    expect(wrapper.get('[data-test="json-rpc-local-endpoint"]').text()).toBe("http://127.0.0.1:17081/jsonrpc");
    expect(wrapper.text()).toContain("17080");
    expect(wrapper.text()).toContain("17081");

    await wrapper.findAll("button").find((button) => button.text() === "配置 Token")!.trigger("click");

    expect(wrapper.emitted("openSettings")).toHaveLength(1);
    expect(wrapper.emitted("update:show")).toContainEqual([false]);
  });

  it("copies the local RPC endpoint without exposing a token", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const { wrapper } = mountWithPinia(AboutDialog, {
      props: {
        show: true,
        appInfo: createAppInfo(),
        updateCheck: null,
      },
    });

    await wrapper.findAll("button").find((button) => button.text() === "复制地址")!.trigger("click");
    await flushPromises();

    expect(writeText).toHaveBeenCalledWith("http://127.0.0.1:17081/jsonrpc");
    expect(wrapper.text()).not.toContain("original-token");
    expect(wrapper.text()).toContain("已复制");
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
