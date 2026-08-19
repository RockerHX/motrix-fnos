import { describe, expect, it, vi } from "vitest";

vi.mock("naive-ui", async () => {
  const { computed, defineComponent, h, inject, provide, toRef } = await import("vue");
  const collapseContextKey = Symbol("help-collapse");

  const NCollapse = defineComponent({
    name: "NCollapseStub",
    inheritAttrs: false,
    props: {
      expandedNames: { type: [String, Array], default: null },
    },
    emits: ["update:expandedNames"],
    setup(props, { attrs, emit, slots }) {
      const expandedNames = toRef(props, "expandedNames");
      provide(collapseContextKey, {
        expandedNames,
        toggle(name: string) {
          emit("update:expandedNames", expandedNames.value === name ? null : name);
        },
      });
      return () => h("div", { ...attrs, "data-test": "n-collapse" }, slots.default?.());
    },
  });

  const NCollapseItem = defineComponent({
    name: "NCollapseItemStub",
    props: {
      name: { type: String, required: true },
      title: { type: String, default: "" },
    },
    setup(props, { slots }) {
      const context = inject<any>(collapseContextKey);
      if (!context) throw new Error("NCollapseItem must be nested in NCollapse");
      const expanded = computed(() => context.expandedNames.value === props.name);
      return () =>
        h("section", { "data-test": "n-collapse-item", "data-topic": props.name }, [
          h("header", [
            h(
              "button",
              {
                type: "button",
                "data-test": "help-collapse-toggle",
                "data-topic": props.name,
                "aria-expanded": String(expanded.value),
                onClick: () => context.toggle(props.name),
              },
              props.title,
            ),
            slots["header-extra"]?.(),
          ]),
          expanded.value ? h("div", { "data-test": "help-collapse-content" }, slots.default?.()) : null,
        ]);
    },
  });

  const slotStub = (name: string) =>
    defineComponent({
      name,
      setup(_, { slots, attrs }) {
        return () => h("span", attrs, slots.default?.());
      },
    });

  return {
    NButton: defineComponent({
      name: "NButtonStub",
      emits: ["click"],
      setup(_, { attrs, emit, slots }) {
        return () => h("button", { ...attrs, onClick: () => emit("click") }, slots.default?.());
      },
    }),
    NCollapse,
    NCollapseItem,
    NTag: slotStub("NTagStub"),
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
        eyebrow: String,
        fixedBody: Boolean,
        contentClass: String,
      },
      emits: ["update:show"],
      setup(props, { emit, slots }) {
        return () =>
          props.show
            ? h("section", { "data-test": "app-dialog", "data-fixed-body": String(props.fixedBody), "data-content-class": props.contentClass }, [
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
  it("opens only the default topic in a fixed-body accordion", () => {
    const { wrapper } = mountWithPinia(HelpDialog, {
      props: {
        show: true,
      },
    });

    expect(wrapper.text()).toContain("Help");
    expect(wrapper.text()).toContain("Motrix 使用帮助");
    expect(wrapper.text()).toContain("授权目录与默认下载目录");
    expect(wrapper.text()).toContain("日志与诊断");
    expect(wrapper.get('[data-test="app-dialog"]').attributes("data-fixed-body")).toBe("true");
    expect(wrapper.get('[data-test="app-dialog"]').attributes("data-content-class")).toBe("help-dialog-content");
    expect(wrapper.find('[data-topic="authorized-dirs"] [data-test="help-collapse-content"]').exists()).toBe(true);
    expect(wrapper.find('[data-topic="diagnostics"] [data-test="help-collapse-content"]').exists()).toBe(false);
    expect(wrapper.text()).toContain("已生效");
    expect(wrapper.text()).toContain("待支持");
    expect(wrapper.text()).toContain("排障入口");
  });

  it("expands one topic at a time and can collapse it", async () => {
    const { wrapper } = mountWithPinia(HelpDialog, { props: { show: true } });

    await toggleTopic(wrapper, "diagnostics");
    expect(wrapper.find('[data-topic="diagnostics"] [data-test="help-collapse-content"]').exists()).toBe(true);
    expect(wrapper.find('[data-topic="authorized-dirs"] [data-test="help-collapse-content"]').exists()).toBe(false);

    await toggleTopic(wrapper, "diagnostics");
    expect(wrapper.find('[data-topic="diagnostics"] [data-test="help-collapse-content"]').exists()).toBe(false);
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

  it("opens the independent RPC guide", async () => {
    const { wrapper } = mountWithPinia(HelpDialog, {
      props: {
        show: true,
      },
    });

    await toggleTopic(wrapper, "json-rpc");
    await wrapper.findAll("button").find((button) => button.text() === "打开配置指南")!.trigger("click");

    expect(wrapper.emitted("openRpcGuide")).toHaveLength(1);
  });
});

async function toggleTopic(wrapper: any, topic: string) {
  await wrapper.get(`[data-test="help-collapse-toggle"][data-topic="${topic}"]`).trigger("click");
  await flushPromises();
}
