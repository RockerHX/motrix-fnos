import { beforeEach, describe, expect, it, vi } from "vitest";

const message = vi.hoisted(() => ({ warning: vi.fn() }));

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return { ...actual, useMessage: () => message };
});

vi.mock("../services/jsonRpcTokenService", () => ({
  getJsonRpcTokenStatus: vi.fn(async () => ({ configured: true, maskedToken: "••••••••abcd" })),
  updateJsonRpcToken: vi.fn(),
}));
vi.mock("../services/lanJsonRpcService", () => ({
  getLanJsonRpcStatus: vi.fn(async () => ({ enabled: true, configured: true, maskedToken: "••••••••1234", port: 17082 })),
  rotateLanJsonRpcToken: vi.fn(),
  updateLanJsonRpcEnabled: vi.fn(),
}));

vi.mock("../../../components/ui/AppDialog.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "AppDialogStub",
      props: { show: Boolean, title: String, eyebrow: String },
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

import JsonRpcGuideDialog from "./JsonRpcGuideDialog.vue";
import { flushPromises, mountWithPinia } from "../../../test/mount";

describe("JsonRpcGuideDialog", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders the independent guide and copies the local endpoint", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const { wrapper } = mountWithPinia(JsonRpcGuideDialog, { props: { show: true } });

    expect(wrapper.text()).toContain("JSON-RPC 配置指南");
    await flushPromises();
    expect(wrapper.get('[data-test="json-rpc-proxy-endpoint"]').text()).toBe("http://127.0.0.1:17081/jsonrpc");
    expect(wrapper.get('[data-test="json-rpc-lan-endpoint"]').text()).toBe("http://<飞牛局域网IP>:17082/jsonrpc");
    expect(wrapper.text()).toContain("公网 Token：已配置");
    expect(wrapper.text()).toContain("局域网入口：已启用，Token：已配置");
    expect(wrapper.text()).toContain("当前远程创建仅支持 HTTP / HTTPS 和 magnet:?");
    expect(wrapper.text()).toContain("ed2k://、thunder:// 不支持");
    await wrapper.findAll("button").find((button) => button.text() === "复制地址")!.trigger("click");
    await flushPromises();

    expect(writeText).toHaveBeenCalledWith("http://127.0.0.1:17081/jsonrpc");
    expect(wrapper.text()).toContain("已复制");
    expect(wrapper.text()).not.toContain("original-token");
  });

  it("switches to settings without leaving two dialogs open", async () => {
    const { wrapper } = mountWithPinia(JsonRpcGuideDialog, { props: { show: true } });

    await wrapper.findAll("button").find((button) => button.text() === "配置 Token")!.trigger("click");

    expect(wrapper.emitted("update:show")).toContainEqual([false]);
    expect(wrapper.emitted("openSettings")).toHaveLength(1);
  });

  it("keeps a manual-copy state and explains why automatic copy is blocked", async () => {
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: undefined });
    Object.defineProperty(document, "execCommand", {
      configurable: true,
      value: vi.fn(() => false),
    });
    const { wrapper } = mountWithPinia(JsonRpcGuideDialog, { props: { show: true } });
    await flushPromises();

    await wrapper.findAll("button").find((button) => button.text() === "复制地址")!.trigger("click");
    await flushPromises();

    expect(wrapper.findAll("button").some((button) => button.text() === "请手动选择复制")).toBe(true);
    expect(message.warning).toHaveBeenCalledWith(
      "当前页面不是可使用剪贴板的安全顶层环境，常见原因是局域网 HTTP 或 fnOS 内嵌窗口。请手动选择内容并按 Ctrl+C / Command+C，或直接打开 Motrix HTTPS 域名。",
    );
  });
});
