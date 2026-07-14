import { describe, expect, it } from "vitest";
import { mountWithPinia } from "../test/mount";
import AppShell from "./AppShell.vue";

describe("AppShell", () => {
  it("shows a persistent protection warning and opens security settings", async () => {
    const { wrapper } = mountWithPinia(AppShell, {
      props: { appInfo: null, activeCategory: "all", protectionEnabled: false },
      slots: { default: "content" },
    });

    expect(wrapper.get('[data-test="protection-warning"]').text()).toContain("允许匿名访问");
    await wrapper.findAll("button").find((button) => button.text() === "立即启用")!.trigger("click");
    expect(wrapper.emitted("enableProtection")).toHaveLength(1);
  });

  it("hides the warning when Web protection is enabled", () => {
    const { wrapper } = mountWithPinia(AppShell, {
      props: { appInfo: null, activeCategory: "all", protectionEnabled: true },
    });

    expect(wrapper.find('[data-test="protection-warning"]').exists()).toBe(false);
  });
});
