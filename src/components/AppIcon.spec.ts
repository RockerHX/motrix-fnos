import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import AppIcon from "./AppIcon.vue";

describe("AppIcon", () => {
  it("renders a known SVG icon with default decorative semantics", () => {
    const wrapper = mount(AppIcon, { props: { name: "download" } });
    const svg = wrapper.get("svg");

    expect(svg.attributes("data-icon-name")).toBe("download");
    expect(svg.attributes("aria-hidden")).toBe("true");
    expect(svg.attributes("role")).toBeUndefined();
  });

  it("normalizes numeric size to px", () => {
    const wrapper = mount(AppIcon, { props: { name: "plus", size: 24 } });
    const svg = wrapper.get("svg");

    expect(svg.attributes("width")).toBe("24px");
    expect(svg.attributes("height")).toBe("24px");
  });

  it("supports non-decorative icon semantics", () => {
    const wrapper = mount(AppIcon, { props: { name: "info", decorative: false } });
    const svg = wrapper.get("svg");

    expect(svg.attributes("role")).toBe("img");
    expect(svg.attributes("aria-label")).toBe("info");
    expect(svg.attributes("aria-hidden")).toBeUndefined();
  });

  it("falls back safely for unknown icon names", () => {
    const wrapper = mount(AppIcon, { props: { name: "missing-icon" } });

    expect(wrapper.get("svg").attributes("data-icon-name")).toBe("unknown");
  });
});
