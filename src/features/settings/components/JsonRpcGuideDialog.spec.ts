import { describe, expect, it, vi } from "vitest";

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
  it("renders the independent guide and copies the local endpoint", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const { wrapper } = mountWithPinia(JsonRpcGuideDialog, { props: { show: true } });

    expect(wrapper.text()).toContain("JSON-RPC 配置指南");
    expect(wrapper.get('[data-test="json-rpc-local-endpoint"]').text()).toBe("http://127.0.0.1:17081/jsonrpc");
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
});
