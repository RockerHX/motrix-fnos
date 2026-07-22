import { ref } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";

const isMobileLayout = ref(false);

vi.mock("../app/composables/useMobileLayout", () => ({
  useMobileLayout: () => ({ isMobileLayout }),
}));

vi.mock("naive-ui", () => ({
  useMessage: () => ({ success: vi.fn(), error: vi.fn() }),
}));

vi.mock("../features/auth/services/authService", () => ({
  getAuthStatus: vi.fn(),
  setupAuth: vi.fn(),
  loginAuth: vi.fn(),
  logoutAuth: vi.fn(async () => undefined),
  changeAuthPassword: vi.fn(),
  changeAuthProtection: vi.fn(),
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
      emits: ["logout", "selectCategory"],
      setup(_, { emit, slots }) {
        return () =>
          h("div", { "data-test": "app-shell" }, [
            h("button", { "data-test": "shell-logout", onClick: () => emit("logout") }, "logout"),
            h("button", { "data-test": "shell-select-all", onClick: () => emit("selectCategory", "all") }, "all"),
            h("button", { "data-test": "shell-select-completed", onClick: () => emit("selectCategory", "completed") }, "completed"),
            h("button", { "data-test": "shell-select-trash", onClick: () => emit("selectCategory", "trash") }, "trash"),
            h("button", { "data-test": "shell-select-extensions", onClick: () => emit("selectCategory", "extensions") }, "extensions"),
            slots.default?.(),
            slots.overlay?.(),
          ]);
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
  return {
    default: defineComponent({
      name: "ExtensionsPlaceholderStub",
      setup: () => () => h("div", { "data-test": "extensions-placeholder" }),
    }),
  };
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
  return {
    default: defineComponent({
      name: "MainWindowDialogsStub",
      props: {
        showAbout: Boolean,
        showSettings: Boolean,
        showJsonRpcGuide: Boolean,
      },
      emits: ["openRpcGuide"],
      setup(props, { emit }) {
        return () =>
          h("div", [
            h("button", { "data-test": "open-rpc-guide", onClick: () => emit("openRpcGuide") }, "open guide"),
            h("span", { "data-test": "main-dialogs-about" }, String(props.showAbout)),
            h("span", { "data-test": "main-dialogs-settings" }, String(props.showSettings)),
            h("span", { "data-test": "main-dialogs-rpc-guide" }, String(props.showJsonRpcGuide)),
          ]);
      },
    }),
  };
});

import MainWindow from "./MainWindow.vue";
import { useTaskStore } from "../features/tasks/stores/taskStore";
import { useAuthStore } from "../features/auth/stores/authStore";
import { flushPromises, mountWithPinia } from "../test/mount";
import type { DownloadTask } from "../types/tasks";

describe("MainWindow floating create button", () => {
  beforeEach(() => {
    isMobileLayout.value = false;
  });

  it("hides floating create button on desktop layout", () => {
    const { wrapper } = mountMainWindow();

    expect(wrapper.find(".floating-add").exists()).toBe(false);
  });

  it("opens the independent RPC guide and closes Settings when requested", async () => {
    const { wrapper } = mountMainWindow();

    await wrapper.get('[data-test="open-rpc-guide"]').trigger("click");
    await wrapper.vm.$nextTick();

    expect(wrapper.get('[data-test="main-dialogs-about"]').text()).toBe("false");
    expect(wrapper.get('[data-test="main-dialogs-settings"]').text()).toBe("false");
    expect(wrapper.get('[data-test="main-dialogs-rpc-guide"]').text()).toBe("true");
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

  it("keeps floating create visibility aligned with mobile category and runtime state", async () => {
    isMobileLayout.value = true;
    const { wrapper } = mountMainWindow();
    const taskStore = useTaskStore();

    expect(wrapper.find(".floating-add").exists()).toBe(false);

    taskStore.tasks = [createTask({ id: 4, gid: "gid-4", status: "active" })];
    await wrapper.vm.$nextTick();
    expect(wrapper.find(".floating-add").exists()).toBe(true);

    await wrapper.get('[data-test="shell-select-completed"]').trigger("click");
    expect(wrapper.find(".floating-add").exists()).toBe(true);

    await wrapper.get('[data-test="shell-select-trash"]').trigger("click");
    expect(wrapper.find(".floating-add").exists()).toBe(false);

    await wrapper.get('[data-test="shell-select-extensions"]').trigger("click");
    expect(wrapper.find(".floating-add").exists()).toBe(false);

    await wrapper.get('[data-test="shell-select-all"]').trigger("click");
    expect(wrapper.find(".floating-add").exists()).toBe(true);

    taskStore.isRuntimeExiting = true;
    await wrapper.vm.$nextTick();
    expect(wrapper.find(".floating-add").exists()).toBe(false);
  });

  it("logs out through the shell and clears sensitive task state", async () => {
    const { wrapper } = mountMainWindow();
    const authStore = useAuthStore();
    authStore.handleUnauthorizedStatus({ setupRequired: false, enabled: true, authenticated: true, csrfToken: "csrf" });
    const taskStore = useTaskStore();
    taskStore.tasks = [{ id: 1 } as never];

    await wrapper.get('[data-test="shell-logout"]').trigger("click");
    await flushPromises();

    expect(authStore.phase).toBe("login");
    expect(taskStore.tasks).toEqual([]);
  });

  it("switches between empty, list and extensions content branches", async () => {
    const { wrapper } = mountMainWindow();
    const taskStore = useTaskStore();

    expect(wrapper.find('[data-test="task-empty"]').exists()).toBe(true);

    taskStore.tasks = [createTask({ id: 2, gid: "gid-2", status: "active" })];
    await wrapper.vm.$nextTick();
    expect(wrapper.find('[data-test="task-table"]').exists()).toBe(true);

    await wrapper.get('[data-test="shell-select-completed"]').trigger("click");
    expect(wrapper.find('[data-test="task-empty"]').exists()).toBe(true);

    await wrapper.get('[data-test="shell-select-trash"]').trigger("click");
    expect(wrapper.find('[data-test="task-empty"]').exists()).toBe(true);

    await wrapper.get('[data-test="shell-select-extensions"]').trigger("click");
    expect(wrapper.find('[data-test="extensions-placeholder"]').exists()).toBe(true);
  });

  it("keeps the task table branch for ordinary task field updates", async () => {
    const { wrapper } = mountMainWindow();
    const taskStore = useTaskStore();
    const task = createTask({ id: 3, gid: "gid-3", status: "active" });

    taskStore.tasks = [task];
    await wrapper.vm.$nextTick();
    const tableElement = wrapper.get('[data-test="task-table"]').element;

    taskStore.tasks = [
      {
        ...task,
        completedLength: 50,
        downloadSpeed: 16,
        errorMessage: "temporary network error",
        updatedAt: 2,
      },
    ];
    await wrapper.vm.$nextTick();

    expect(wrapper.get('[data-test="task-table"]').element).toBe(tableElement);
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

function createTask(overrides: Partial<DownloadTask> = {}): DownloadTask {
  return {
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
    ...overrides,
  };
}
