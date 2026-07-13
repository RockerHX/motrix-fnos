import { defineComponent, h, ref } from "vue";
import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { naiveUiStubs } from "../../../test/mount";

const focus = vi.hoisted(() => vi.fn());
const select = vi.hoisted(() => vi.fn());

vi.mock("naive-ui", () => ({
  ...naiveUiStubs,
  NInput: defineComponent({
    props: { value: String, readonly: Boolean, inputProps: Object },
    setup(props, { expose }) {
      const textareaElRef = ref<HTMLTextAreaElement | null>(null);
      expose({ focus, select, textareaElRef });
      return () => h("textarea", {
        ref: textareaElRef,
        "data-test": "manual-copy-text",
        value: props.value,
        readonly: props.readonly || Boolean((props.inputProps as { readonly?: boolean })?.readonly),
      });
    },
  }),
}));

import DebugLogManualCopyDialog from "./DebugLogManualCopyDialog.vue";

describe("DebugLogManualCopyDialog", () => {
  it("shows readonly text, focuses it and emits download and close", async () => {
    const wrapper = mount(DebugLogManualCopyDialog, {
      props: { show: false, text: "debug log text" },
    });

    await wrapper.setProps({ show: true });
    await wrapper.vm.$nextTick();

    const textarea = wrapper.get<HTMLTextAreaElement>('[data-test="manual-copy-text"]');
    expect(textarea.element.readOnly).toBe(true);
    expect(textarea.element.value).toBe("debug log text");
    expect(focus).toHaveBeenCalled();
    expect(select).toHaveBeenCalled();

    await clickButton(wrapper, "下载日志");
    await clickButton(wrapper, "完成");
    expect(wrapper.emitted("download")).toHaveLength(1);
    expect(wrapper.emitted("update:show")).toContainEqual([false]);
  });
});

async function clickButton(wrapper: ReturnType<typeof mount>, text: string) {
  const button = wrapper.findAll("button").find((item) => item.text() === text);
  if (!button) throw new Error(`button not found: ${text}`);
  await button.trigger("click");
}
