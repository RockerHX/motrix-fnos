import { describe, expect, it, vi } from "vitest";

vi.mock("naive-ui", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    NSwitch: defineComponent({
      name: "NSwitchStub",
      inheritAttrs: false,
      props: {
        value: { type: Boolean, default: false },
        disabled: { type: Boolean, default: false },
        loading: { type: Boolean, default: false },
      },
      emits: ["update:value"],
      setup(props, { emit, attrs }) {
        return () =>
          h("button", {
            ...attrs,
            type: "button",
            role: "switch",
            disabled: props.disabled,
            "aria-checked": String(props.value),
            "data-loading": String(props.loading),
            onClick: () => {
              if (!props.disabled) emit("update:value", !props.value);
            },
          });
      },
    }),
  };
});

import { mount } from "@vue/test-utils";
import TaskProxyToggle from "./TaskProxyToggle.vue";

describe("TaskProxyToggle", () => {
  it("renders a controlled proxy value and forwards changes", async () => {
    const wrapper = mount(TaskProxyToggle, {
      props: { value: true },
    });
    const proxySwitch = wrapper.get('button[role="switch"]');

    expect(wrapper.text()).toContain("已开启");
    expect(proxySwitch.attributes("aria-checked")).toBe("true");
    await proxySwitch.trigger("click");

    expect(wrapper.emitted("update:value")).toEqual([[false]]);
    expect(proxySwitch.attributes("aria-checked")).toBe("true");
  });

  it("disables repeated changes while a task operation is running", () => {
    const wrapper = mount(TaskProxyToggle, {
      props: { value: false, disabled: true, loading: true },
    });
    const proxySwitch = wrapper.get('button[role="switch"]');

    expect(proxySwitch.attributes("disabled")).toBeDefined();
    expect(proxySwitch.attributes("data-loading")).toBe("true");
  });
});
