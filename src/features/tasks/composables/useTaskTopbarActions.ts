import { computed, type Ref } from "vue";
import { useTaskStore } from "../stores/taskStore";
import { useTaskToolbar } from "./useTaskToolbar";
import type { TranslationKey, TranslationParams } from "../../../i18n";
import type { MainNavCategory } from "../../../types/navigation";
import type { TopbarActionStates } from "../../../types/topbar";

interface UseTaskTopbarActionsOptions {
  activeCategory: Ref<MainNavCategory>;
  taskStore: ReturnType<typeof useTaskStore>;
  toolbar: ReturnType<typeof useTaskToolbar>;
  refreshTasks: (showError?: boolean) => Promise<void>;
  refreshRemovedTasks: (showError?: boolean) => Promise<void>;
  refreshAria2Status: () => Promise<unknown>;
  t: (key: TranslationKey, params?: TranslationParams) => string;
}

export function useTaskTopbarActions(options: UseTaskTopbarActionsOptions) {
  const { activeCategory, taskStore, toolbar, refreshTasks, refreshRemovedTasks, refreshAria2Status, t } = options;
  const topbarActions = computed<TopbarActionStates>(() => ({
    create: state(toolbar.canCreate.value, "topbar.create", createDisabledTitle()),
    refresh: state(toolbar.canRefresh.value, "common.refresh", refreshDisabledTitle()),
    pauseVisible: state(toolbar.canPauseVisible.value, "topbar.pauseVisible", batchDisabledTitle("pause")),
    resumeVisible: state(toolbar.canResumeVisible.value, "topbar.resumeVisible", batchDisabledTitle("resume")),
    deleteVisible: state(toolbar.canDeleteVisible.value, "topbar.deleteVisible", batchDisabledTitle("delete")),
    clearTrash: state(toolbar.canClearTrash.value, "topbar.clearTrash", clearTrashDisabledTitle()),
  }));

  function state(enabled: boolean, enabledKey: TranslationKey, disabledTitle: string) {
    return { disabled: !enabled, title: enabled ? t(enabledKey) : disabledTitle };
  }

  function runtimeOrExtensionsTitle(fallback: TranslationKey) {
    if (taskStore.isRuntimeExiting) return t("topbar.disabled.runtimeExiting");
    if (activeCategory.value === "extensions") return t("topbar.disabled.extensions");
    return t(fallback);
  }

  function createDisabledTitle() {
    return runtimeOrExtensionsTitle("topbar.create");
  }

  function refreshDisabledTitle() {
    return runtimeOrExtensionsTitle("common.refresh");
  }

  function clearTrashDisabledTitle() {
    return taskStore.isRuntimeExiting ? t("topbar.disabled.runtimeExiting") : t("topbar.disabled.trashEmpty");
  }

  function batchDisabledTitle(action: "pause" | "resume" | "delete") {
    const keys = { pause: "topbar.disabled.noPauseable", resume: "topbar.disabled.noResumable", delete: "topbar.disabled.noDeletable" } as const;
    return runtimeOrExtensionsTitle(keys[action]);
  }

  async function refresh() {
    if (!toolbar.canRefresh.value) return;
    // 引擎状态与任务列表互不依赖，并行触发可避免状态探测延迟阻塞用户主动刷新任务。
    void refreshAria2Status();
    if (activeCategory.value === "trash") {
      await refreshRemovedTasks(true);
      return;
    }
    await refreshTasks(true);
  }

  return { topbarActions, refresh };
}
