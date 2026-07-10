import { ref } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";

const isMobileLayout = ref(false);

vi.mock("../app/composables/useMobileLayout", () => ({
  useMobileLayout: () => ({ isMobileLayout }),
}));

vi.mock("naive-ui", () => ({
  useMessage: () => ({ success: vi.fn(), error: vi.fn() }),
}));

vi.mock("../features/about/composables/useUpdateCheck", () => ({
  useUpdateCheck: () => ({ updateCheck: ref(null), isCheckingUpdate: ref(false), runUpdateCheck: vi.fn() }),
}));

vi.mock("../features/diagnostics/composables/useAria2Status", () => ({
  useAria2Status: () => ({
    aria2Process: ref(null),
    aria2Rpc: ref(null),
    refreshAria2Status: vi.fn(),
    updateAria2Status: vi.fn(),
  }),
}));

vi.mock("../features/tasks/composables/useTaskToasts", () => ({
  useTaskToasts: () => ({ refreshTasks: vi.fn(), refreshRemovedTasks: vi.fn(), flushTaskErrorMessages: vi.fn() }),
}));

vi.mock("../layouts/AppShell.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "AppShellStub",
      setup(_, { slots }) {
        return () => h("div", { "data-test": "app-shell" }, [slots.default?.(), slots.overlay?.()]);
      },
    }),
  };
});

vi.mock("../features/tasks/components/TaskEmptyState.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "TaskEmptyStateStub",
      setup() {
        return () => h("div", { "data-test": "task-empty" });
      },
    }),
  };
});

vi.mock("../features/tasks/components/TaskTable.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "TaskTableStub",
      setup() {
        return () => h("div", { "data-test": "task-table" });
      },
    }),
  };
});

vi.mock("../features/extensions/components/ExtensionsPlaceholder.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return { default: defineComponent({ name: "ExtensionsPlaceholderStub", setup: () => () => h("div") }) };
});

vi.mock("../features/tasks/components/TaskBulkDeleteConfirmDialog.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return { default: defineComponent({ name: "TaskBulkDeleteConfirmDialogStub", setup: () => () => h("div") }) };
});

vi.mock("../features/tasks/components/TaskFileConfirmCoordinator.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return { default: defineComponent({ name: "TaskFileConfirmCoordinatorStub", setup: () => () => h("div") }) };
});

vi.mock("./MainWindowDialogs.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return { default: defineComponent({ name: "MainWindowDialogsStub", setup: () => () => h("div") }) };
});

import MainWindow from "./MainWindow.vue";
import { useTaskStore } from "../features/tasks/stores/taskStore";
import { mountWithPinia } from "../test/mount";

describe("MainWindow floating create button", () => {
  beforeEach(() => {
    isMobileLayout.value = false;
  });

  it("hides floating create button on desktop layout", () => {
    const { wrapper } = mountMainWindow();

    expect(wrapper.find(".floating-add").exists()).toBe(false);
  });

  it("keeps floating create button on mobile layout when the task list is visible", async () => {
    isMobileLayout.value = true;
    const { wrapper } = mountMainWindow();
    const taskStore = useTaskStore();

    taskStore.tasks = [
      {
        id: 1,
        url: "https://example.com/file.iso",
        fileName: "file.iso",
        saveDir: "/downloads",
        category: "downloading",
        gid: "gid-1",
        status: "active",
        totalLength: 100,
        completedLength: 10,
        downloadSpeed: 1,
        errorCode: null,
        errorMessage: null,
        filePath: null,
        metadataTorrentPath: null,
        confirmationRequired: false,
        files: [],
        createdAt: 0,
        updatedAt: 0,
      },
    ];
    await wrapper.vm.$nextTick();

    expect(wrapper.find(".floating-add").exists()).toBe(true);
  });
});

function mountMainWindow() {
  return mountWithPinia(MainWindow, {
    props: {
      appInfo: null,
      backendPing: null,
      errorMessage: "",
    },
    global: {
      stubs: {
        AppIcon: true,
      },
    },
  });
}
