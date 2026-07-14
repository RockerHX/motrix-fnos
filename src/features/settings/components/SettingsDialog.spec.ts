import { describe, expect, it, vi } from "vitest";

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
    useMessage: () => ({ success: vi.fn(), error: vi.fn() }),
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
}));

vi.mock("../../auth/components/WebAuthSettings.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return { default: defineComponent({ name: "WebAuthSettingsStub", setup: () => () => h("div", "Web 管理安全") }) };
});

vi.mock("./JsonRpcTokenSettings.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return { default: defineComponent({ name: "JsonRpcTokenSettingsStub", setup: () => () => h("div", "JSON-RPC Token 专用设置") }) };
});

import SettingsDialog from "./SettingsDialog.vue";
import { flushPromises, mountWithPinia } from "../../../test/mount";

describe("SettingsDialog", () => {
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
});
