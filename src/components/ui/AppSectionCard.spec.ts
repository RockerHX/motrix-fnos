import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import AppSectionCard from "./AppSectionCard.vue";

describe("AppSectionCard", () => {
  it("renders title description and default content from props", () => {
    const wrapper = mount(AppSectionCard, {
      props: {
        title: "更新检查",
        description: "手动检查新版本",
      },
      slots: {
        default: '<div data-test="content">区块内容</div>',
      },
    });

    expect(wrapper.classes()).toContain("app-section-card");
    expect(wrapper.get(".app-section-card__title").text()).toBe("更新检查");
    expect(wrapper.get(".app-section-card__description").text()).toBe("手动检查新版本");
    expect(wrapper.get('[data-test="content"]').text()).toBe("区块内容");
  });

  it("allows title meta actions and default slots", () => {
    const wrapper = mount(AppSectionCard, {
      slots: {
        title: "自定义标题",
        meta: '<span data-test="meta">状态</span>',
        actions: '<button data-test="action">操作</button>',
        default: '<p data-test="body">正文</p>',
      },
    });

    expect(wrapper.get(".app-section-card__title").text()).toBe("自定义标题");
    expect(wrapper.get('[data-test="meta"]').text()).toBe("状态");
    expect(wrapper.get('[data-test="action"]').text()).toBe("操作");
    expect(wrapper.get('[data-test="body"]').text()).toBe("正文");
  });
});
