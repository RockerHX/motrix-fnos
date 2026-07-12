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

vi.mock("./TaskDesktopList.vue", () => ({
  default: {
    name: "TaskDesktopListStub",
    props: ["tasks"],
    template: '<div data-test="task-desktop-list">{{ tasks.length }}</div>',
  },
}));

import TaskTable from "./TaskTable.vue";
import { mountWithPinia } from "../../../test/mount";

describe("TaskTable", () => {
  it("renders desktop list when not in mobile layout", () => {
    isMobileLayout.value = false;
    const { wrapper } = mountWithPinia(TaskTable, {
      props: {
        tasks: [{ id: 1 }],
        ...paginationProps(),
      },
    });

    expect(wrapper.find('[data-test="task-desktop-list"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="task-mobile-list"]').exists()).toBe(false);
  });

  it("renders mobile list when in mobile layout", () => {
    isMobileLayout.value = true;
    const { wrapper } = mountWithPinia(TaskTable, {
      props: {
        tasks: [{ id: 1 }],
        ...paginationProps(),
      },
    });

    expect(wrapper.find('[data-test="task-mobile-list"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="task-desktop-list"]').exists()).toBe(false);
  });

  it("emits desktop pagination and page-size changes", async () => {
    isMobileLayout.value = false;
    const { wrapper } = mountWithPinia(TaskTable, {
      props: {
        tasks: [{ id: 1 }],
        ...paginationProps({ showPagination: true, itemCount: 101 }),
      },
    });

    const pagination = wrapper.getComponent({ name: "Pagination" });
    pagination.vm.$emit("update:page", 2);
    pagination.vm.$emit("update:page-size", 50);

    expect(wrapper.emitted("update:page")).toEqual([[2]]);
    expect(wrapper.emitted("update:pageSize")).toEqual([[50]]);
  });
});

function paginationProps(overrides: Record<string, unknown> = {}) {
  return {
    page: 1,
    pageSize: 20,
    itemCount: 1,
    showPagination: false,
    ...overrides,
  };
}
