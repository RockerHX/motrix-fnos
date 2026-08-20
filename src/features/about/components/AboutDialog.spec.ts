import { describe, expect, it, vi } from "vitest";

vi.mock("naive-ui", async () => {
  const { computed, defineComponent, h, inject, provide, ref, toRef } = await import("vue");
  const tabsContextKey = Symbol("about-tabs");

  const NTabPane = defineComponent({
    name: "NTabPaneStub",
    props: {
      name: { type: String, required: true },
      tab: { type: String, default: "" },
    },
    setup(props, { slots }) {
      const context = inject<any>(tabsContextKey);
      if (!context) throw new Error("NTabPane must be nested in NTabs");
      const active = computed(() => context.value.value === props.name);
      const rendered = computed(() => active.value || context.rendered.value.has(props.name));
      return () => {
        if (active.value) context.rendered.value.add(props.name);
        if (!rendered.value) return null;
        return h(
          "div",
          {
            "data-test": "n-tab-pane",
            "data-pane": props.name,
            style: { display: active.value ? "" : "none" },
          },
          slots.default?.(),
        );
      };
    },
  });

  const NTabs = defineComponent({
    name: "NTabsStub",
    inheritAttrs: false,
    props: {
      value: { type: String, default: "" },
    },
    emits: ["update:value"],
    setup(props, { attrs, emit, slots }) {
      const rendered = ref(new Set<string>());
      const value = toRef(props, "value");
      provide(tabsContextKey, { value, rendered });
      return () => {
        const panes = (slots.default?.() ?? []).filter((vnode) => typeof vnode.type === "object");
        return h("div", { ...attrs, "data-test": "n-tabs" }, [
          h(
            "nav",
            { "data-test": "n-tabs-nav" },
            panes.map((pane) =>
              h(
                "button",
                {
                  type: "button",
                  "data-tab": pane.props?.name,
                  onClick: () => emit("update:value", pane.props?.name),
                },
                pane.props?.tab,
              ),
            ),
          ),
          h("div", { "data-test": "n-tabs-content" }, slots.default?.()),
        ]);
      };
    },
  });

  const slotStub = (name: string) =>
    defineComponent({
      name,
      setup(_, { slots }) {
        return () => h("div", slots.default?.());
      },
    });

  return {
    NButton: defineComponent({
      name: "NButtonStub",
      props: { loading: { type: Boolean, default: false } },
      emits: ["click"],
      setup(_, { attrs, emit, slots }) {
        return () => h("button", { ...attrs, onClick: () => emit("click") }, slots.default?.());
      },
    }),
    NDescriptions: slotStub("NDescriptionsStub"),
    NDescriptionsItem: slotStub("NDescriptionsItemStub"),
    NTabPane,
    NTabs,
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

import AboutDialog from "./AboutDialog.vue";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import type { AppInfo } from "../../../types/app";

describe("AboutDialog", () => {
  it("renders the overview by default with fixed dialog content", () => {
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
    expect(wrapper.get('[data-test="app-dialog"]').attributes("data-fixed-body")).toBe("true");
    expect(wrapper.get('[data-test="app-dialog"]').attributes("data-content-class")).toBe("about-dialog-content");
    expect(wrapper.find('[data-pane="overview"]').isVisible()).toBe(true);
    expect(wrapper.find('[data-pane="changelog"]').exists()).toBe(false);
  });

  it("lazily mounts and preserves the changelog tab", async () => {
    const { wrapper } = mountWithPinia(AboutDialog, {
      props: {
        show: true,
        appInfo: createAppInfo(),
        updateCheck: null,
      },
    });

    await selectTab(wrapper, "更新记录");
    expect(wrapper.text()).toContain("更新历史");
    expect(wrapper.find('[data-pane="changelog"]').attributes("style") ?? "").not.toContain("display: none");
    await selectTab(wrapper, "概览");
    expect(wrapper.find('[data-pane="changelog"]').exists()).toBe(true);
    expect(wrapper.find('[data-pane="changelog"]').attributes("style") ?? "").toContain("display: none");
  });

  it("emits close and checkUpdate events", async () => {
    const { wrapper } = mountWithPinia(AboutDialog, {
      props: {
        show: true,
        appInfo: createAppInfo(),
        updateCheck: null,
      },
    });

    await selectTab(wrapper, "概览");
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

    await selectTab(wrapper, "概览");
    expect(wrapper.text()).toContain("JSON-RPC 使用指南");
    await wrapper.findAll("button").find((button) => button.text() === "查看指南")!.trigger("click");

    expect(wrapper.emitted("openRpcGuide")).toHaveLength(1);
    expect(wrapper.emitted("update:show")).toContainEqual([false]);
  });
});

async function selectTab(wrapper: any, label: string) {
  await wrapper.get(".about-tabs").findAll("button").find((button: any) => button.text() === label)!.trigger("click");
  await flushPromises();
}

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
