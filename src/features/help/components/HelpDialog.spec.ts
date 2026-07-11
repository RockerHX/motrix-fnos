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

import HelpDialog from "./HelpDialog.vue";
import { flushPromises, mountWithPinia } from "../../../test/mount";

describe("HelpDialog", () => {
  it("renders help sections", () => {
    const { wrapper } = mountWithPinia(HelpDialog, {
      props: {
        show: true,
      },
    });

    expect(wrapper.text()).toContain("Help");
    expect(wrapper.text()).toContain("Motrix 使用帮助");
    expect(wrapper.text()).toContain("授权目录与默认下载目录");
    expect(wrapper.text()).toContain("日志与诊断");
  });

  it("emits close event", async () => {
    const { wrapper } = mountWithPinia(HelpDialog, {
      props: {
        show: true,
      },
    });

    await wrapper.get('button[aria-label="关闭"]').trigger("click");
    await flushPromises();

    expect(wrapper.emitted("update:show")).toEqual([[false]]);
  });
});
