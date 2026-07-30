import { describe, expect, it, vi } from "vitest";
import { computed, reactive, ref, type Ref } from "vue";
import { useTaskTopbarActions } from "./useTaskTopbarActions";
import type { MainNavCategory } from "../../../types/navigation";

describe("useTaskTopbarActions", () => {
  it("uses runtime and extensions disabled titles", () => {
    const activeCategory = ref<MainNavCategory>("extensions");
    const taskStore = reactive({ isRuntimeExiting: false });
    const actions = createActions(activeCategory, taskStore);
    expect(actions.topbarActions.value.create!.title).toBe("topbar.disabled.extensions");

    taskStore.isRuntimeExiting = true;
    expect(actions.topbarActions.value.refresh!.title).toBe("topbar.disabled.runtimeExiting");
  });

  it("refreshes removed tasks in Trash and normal tasks elsewhere", async () => {
    const activeCategory = ref<MainNavCategory>("trash");
    const refreshTasks = vi.fn();
    const refreshRemovedTasks = vi.fn();
    const refreshAria2Status = vi.fn();
    const actions = createActions(activeCategory, { isRuntimeExiting: false }, { refreshTasks, refreshRemovedTasks, refreshAria2Status });

    await actions.refresh();
    expect(refreshRemovedTasks).toHaveBeenCalledWith(true);
    expect(refreshAria2Status).toHaveBeenCalled();
    activeCategory.value = "downloading";
    await actions.refresh();
    expect(refreshTasks).toHaveBeenCalledWith(true);
  });

  it("manual refresh reads active tasks and Aria2 status once", async () => {
    const activeCategory = ref<MainNavCategory>("downloading");
    const refreshTasks = vi.fn();
    const refreshRemovedTasks = vi.fn();
    const refreshAria2Status = vi.fn();
    const actions = createActions(
      activeCategory,
      { isRuntimeExiting: false },
      { refreshTasks, refreshRemovedTasks, refreshAria2Status },
    );

    await actions.refresh();

    expect(refreshTasks).toHaveBeenCalledTimes(1);
    expect(refreshTasks).toHaveBeenCalledWith(true);
    expect(refreshRemovedTasks).not.toHaveBeenCalled();
    expect(refreshAria2Status).toHaveBeenCalledTimes(1);
  });
});

function createActions(activeCategory: Ref<MainNavCategory>, taskStore: { isRuntimeExiting: boolean }, overrides = {}) {
  const enabled = ref(false);
  return useTaskTopbarActions({
    activeCategory,
    taskStore: taskStore as never,
    toolbar: {
      canCreate: enabled,
      canRefresh: computed(() => !taskStore.isRuntimeExiting && activeCategory.value !== "extensions"),
      canPauseVisible: enabled,
      canResumeVisible: enabled,
      canDeleteVisible: enabled,
      canClearTrash: enabled,
    } as never,
    refreshTasks: vi.fn(), refreshRemovedTasks: vi.fn(), refreshAria2Status: vi.fn(),
    t: (key) => key,
    ...overrides,
  });
}
