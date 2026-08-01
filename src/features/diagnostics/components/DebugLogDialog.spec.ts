import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { flushPromises, mountWithPinia, naiveUiStubs } from "../../../test/mount";
import type { DebugLogEntry } from "../types";

const messageApi = vi.hoisted(() => ({
  success: vi.fn(),
  warning: vi.fn(),
  error: vi.fn(),
}));

const debugLogService = vi.hoisted(() => ({
  listDebugLogs: vi.fn(),
  clearDebugLogs: vi.fn(),
}));

vi.mock("naive-ui", async () => {
  const { defineComponent, h, ref } = await import("vue");

  return {
    ...naiveUiStubs,
    NModal: defineComponent({
      name: "NModalStub",
      props: { show: { type: Boolean, default: false } },
      emits: ["update:show"],
      setup(props, { emit, slots }) {
        return () =>
          props.show
            ? h(
                "div",
                {
                  "data-test": "n-modal",
                  onClick: (event: MouseEvent) => {
                    if (event.target === event.currentTarget) {
                      emit("update:show", false);
                    }
                  },
                },
                slots.default?.(),
              )
            : null;
      },
    }),
    NCard: defineComponent({
      name: "NCardStub",
      setup(_, { slots }) {
        return () =>
          h("div", { "data-test": "n-card" }, [
            ...(slots.header?.() ?? []),
            ...(slots["header-extra"]?.() ?? []),
            ...(slots.default?.() ?? []),
            ...(slots.footer?.() ?? []),
          ]);
      },
    }),
    NEmpty: defineComponent({
      name: "NEmptyStub",
      props: {
        description: {
          type: String,
          default: "",
        },
      },
      setup(props) {
        return () => h("div", { "data-test": "n-empty" }, props.description);
      },
    }),
    NInput: defineComponent({
      name: "NInputStub",
      props: {
        value: {
          type: String,
          default: "",
        },
        type: {
          type: String,
          default: "text",
        },
        readonly: {
          type: Boolean,
          default: false,
        },
        inputProps: {
          type: Object,
          default: () => ({}),
        },
        placeholder: {
          type: String,
          default: "",
        },
      },
      emits: ["update:value"],
      setup(props, { emit, expose }) {
        const inputRef = ref<HTMLInputElement | HTMLTextAreaElement | null>(null);
        expose({
          focus: () => inputRef.value?.focus(),
          select: () => inputRef.value?.select(),
          textareaElRef: inputRef,
          inputElRef: inputRef,
        });

        return () =>
          props.type === "textarea"
            ? h("textarea", {
                ref: inputRef,
                "data-test": "n-input-textarea",
                value: props.value,
                readonly: props.readonly || Boolean((props.inputProps as { readonly?: boolean }).readonly),
              })
            : h("input", {
                ref: inputRef,
                value: props.value,
                placeholder: props.placeholder,
                readonly: props.readonly,
                onInput: (event: Event) => {
                  emit("update:value", (event.target as HTMLInputElement).value);
                },
              });
      },
    }),
    NSwitch: defineComponent({
      name: "NSwitchStub",
      props: {
        value: {
          type: Boolean,
          default: false,
        },
        size: {
          type: String,
          default: undefined,
        },
      },
      emits: ["update:value"],
      setup(props, { emit }) {
        return () =>
          h("input", {
            type: "checkbox",
            checked: props.value,
            onChange: (event: Event) => {
              emit("update:value", (event.target as HTMLInputElement).checked);
            },
          });
      },
    }),
    NTag: defineComponent({
      name: "NTagStub",
      setup(_, { slots }) {
        return () => h("span", { "data-test": "n-tag" }, slots.default?.());
      },
    }),
    useMessage: () => messageApi,
  };
});

vi.mock("../services/debugLogService", () => debugLogService);

import DebugLogDialog from "./DebugLogDialog.vue";

describe("DebugLogDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    debugLogService.listDebugLogs.mockResolvedValue([createLogEntry()]);
    debugLogService.clearDebugLogs.mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: vi.fn().mockRejectedValue(new Error("denied")),
      },
    });
    Object.defineProperty(document, "execCommand", {
      configurable: true,
      value: vi.fn(() => false),
    });
  });

  it("refreshes when opened and forwards a modal hide request", async () => {
    const { wrapper } = mountWithPinia(DebugLogDialog, {
      props: { show: false },
    });

    await wrapper.setProps({ show: true });
    await flushPromises();
    expect(debugLogService.listDebugLogs).toHaveBeenCalledOnce();

    await wrapper.get('[data-test="n-modal"]').trigger("click");

    expect(wrapper.emitted("update:show")).toEqual([[false]]);
  });

  it("reports a successful clear exactly once", async () => {
    const { wrapper } = mountWithPinia(DebugLogDialog, {
      props: { show: true },
    });
    await flushPromises();

    await clickButton(wrapper, "清空");
    await flushPromises();

    expect(debugLogService.clearDebugLogs).toHaveBeenCalledOnce();
    expect(messageApi.success).toHaveBeenCalledOnce();
  });

  it("allows safe close while clearing is in flight", async () => {
    let resolveClear!: () => void;
    debugLogService.clearDebugLogs.mockReturnValueOnce(new Promise<void>((resolve) => {
      resolveClear = resolve;
    }));
    const { wrapper } = mountWithPinia(DebugLogDialog, {
      props: { show: true },
    });
    await flushPromises();

    await clickButton(wrapper, "清空");
    await nextTick();
    await wrapper.get('[data-test="n-modal"]').trigger("click");

    expect(wrapper.emitted("update:show")).toEqual([[false]]);
    resolveClear();
    await flushPromises();
  });

  it("shows manual copy content with NInput textarea when clipboard copy fails", async () => {
    const { wrapper } = mountWithPinia(DebugLogDialog, {
      props: {
        show: false,
      },
    });

    await wrapper.setProps({ show: true });
    await flushPromises();
    await nextTick();

    await clickButton(wrapper, "复制全部");
    await flushPromises();
    await nextTick();

    const manualCopyTextarea = wrapper.get('[data-test="n-input-textarea"]');
    expect(manualCopyTextarea.element.tagName).toBe("TEXTAREA");
    expect((manualCopyTextarea.element as HTMLTextAreaElement).readOnly).toBe(true);
    expect((manualCopyTextarea.element as HTMLTextAreaElement).value).toContain("aria2.rpc");
    expect((manualCopyTextarea.element as HTMLTextAreaElement).value).toContain("rpc failed");
    expect(messageApi.warning).toHaveBeenCalledWith(
      "当前页面不是可使用剪贴板的安全顶层环境，常见原因是局域网 HTTP 或 fnOS 内嵌窗口。请手动选择内容并按 Ctrl+C / Command+C，或直接打开 Motrix HTTPS 域名。",
    );
    expect(navigator.clipboard.writeText).toHaveBeenCalledOnce();
    expect(wrapper.emitted("update:show")).toBeUndefined();
  });
});

function createLogEntry(): DebugLogEntry {
  return {
    id: 1,
    timestampMs: 1_700_000_000_000,
    lastTimestampMs: 1_700_000_000_000,
    level: "error",
    category: "aria2",
    module: "aria2.rpc",
    message: "rpc failed",
    repeatCount: 1,
  };
}

async function clickButton(wrapper: ReturnType<typeof mountWithPinia>["wrapper"], text: string) {
  const button = wrapper.findAll("button").find((item) => item.text() === text);
  if (!button) {
    throw new Error(`button not found: ${text}`);
  }
  await button.trigger("click");
}
