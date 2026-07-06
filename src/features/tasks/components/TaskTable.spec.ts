import { ref } from "vue";
import { describe, expect, it, vi } from "vitest";

const isMobileLayout = ref(false);

vi.mock("../../../app/composables/useMobileLayout", () => ({
  useMobileLayout: () => ({
    isMobileLayout,
  }),
}));

vi.mock("./TaskMobileList.vue", () => ({
  default: {
    name: "TaskMobileListStub",
    props: ["tasks"],
    template: '<div data-test="task-mobile-list">{{ tasks.length }}</div>',
  },
}));

vi.mock("./TaskDesktopTable.vue", () => ({
  default: {
    name: "TaskDesktopTableStub",
    props: ["tasks"],
    template: '<div data-test="task-desktop-table">{{ tasks.length }}</div>',
  },
}));

import TaskTable from "./TaskTable.vue";
import { mountWithPinia } from "../../../test/mount";

describe("TaskTable", () => {
  it("renders desktop table when not in mobile layout", () => {
    isMobileLayout.value = false;
    const { wrapper } = mountWithPinia(TaskTable, {
      props: {
        tasks: [{ id: 1 }],
      },
    });

    expect(wrapper.find('[data-test="task-desktop-table"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="task-mobile-list"]').exists()).toBe(false);
  });

  it("renders mobile list when in mobile layout", () => {
    isMobileLayout.value = true;
    const { wrapper } = mountWithPinia(TaskTable, {
      props: {
        tasks: [{ id: 1 }],
      },
    });

    expect(wrapper.find('[data-test="task-mobile-list"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="task-desktop-table"]').exists()).toBe(false);
  });
});
