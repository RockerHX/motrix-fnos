import { defineComponent, h } from "vue";
import { describe, expect, it, vi } from "vitest";
import { createPinia, defineStore } from "pinia";
import { createEventSourceMock, mountWithPinia } from "./mount";

describe("mountWithPinia", () => {
  it("injects a usable pinia instance", () => {
    const useCounterStore = defineStore("counter-test", {
      state: () => ({
        count: 1,
      }),
    });

    const TestComponent = defineComponent({
      setup() {
        const counter = useCounterStore();
        return () => h("p", counter.count);
      },
    });

    const { wrapper, pinia } = mountWithPinia(TestComponent, {
      pinia: createPinia(),
    });

    expect(pinia).toBeTruthy();
    expect(wrapper.text()).toContain("1");
  });
});

describe("createEventSourceMock", () => {
  it("records listeners and emits events", () => {
    const { EventSourceMock, instances } = createEventSourceMock();
    const source = EventSourceMock("/api/events");
    const listener = vi.fn();

    source.addEventListener("tasks.snapshot", listener);
    source.emit("tasks.snapshot", new MessageEvent("tasks.snapshot", { data: "{\"tasks\":[]}" }));

    expect(instances).toHaveLength(1);
    expect(listener).toHaveBeenCalledTimes(1);
    expect(instances[0]?.url).toBe("/api/events");
  });
});
