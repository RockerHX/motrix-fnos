import { mount } from "@vue/test-utils";
import { defineComponent, h, nextTick, type Ref } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useMobileLayout } from "./useMobileLayout";

describe("useMobileLayout", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("reads the initial media query and reacts to changes", async () => {
    const media = mediaQuery(true);
    vi.spyOn(window, "matchMedia").mockReturnValue(media.query);
    let isMobileLayout!: Ref<boolean>;

    const wrapper = mount(defineComponent({
      setup() {
        ({ isMobileLayout } = useMobileLayout());
        return () => h("div");
      },
    }));

    expect(window.matchMedia).toHaveBeenCalledWith("(max-width: 767px)");
    expect(isMobileLayout.value).toBe(true);
    media.emit(false);
    await nextTick();
    expect(isMobileLayout.value).toBe(false);

    wrapper.unmount();
    expect(media.query.removeEventListener).toHaveBeenCalledWith("change", expect.any(Function));
  });

  it("keeps desktop layout when matchMedia is unavailable", () => {
    const original = window.matchMedia;
    Object.defineProperty(window, "matchMedia", { configurable: true, writable: true, value: undefined });
    let isMobileLayout!: Ref<boolean>;

    const wrapper = mount(defineComponent({
      setup() {
        ({ isMobileLayout } = useMobileLayout());
        return () => h("div");
      },
    }));

    expect(isMobileLayout.value).toBe(false);
    wrapper.unmount();
    Object.defineProperty(window, "matchMedia", { configurable: true, writable: true, value: original });
  });
});

function mediaQuery(initialMatches: boolean) {
  let listener: ((event: MediaQueryListEvent) => void) | undefined;
  const query = {
    media: "(max-width: 767px)",
    matches: initialMatches,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn((type: string, callback: (event: MediaQueryListEvent) => void) => {
      if (type === "change") listener = callback;
    }),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  } as unknown as MediaQueryList;

  return {
    query,
    emit(matches: boolean) {
      listener?.({ matches } as MediaQueryListEvent);
    },
  };
}
