import { computed, type Ref } from "vue";
import type { MainNavCategory } from "../../../types/navigation";
import type { DownloadTask } from "../../../types/tasks";

interface UseTaskToolbarOptions {
  activeCategory: Ref<MainNavCategory>;
  isRuntimeExiting: Ref<boolean>;
  visibleTasks?: Ref<DownloadTask[]>;
  clearTrashTasks?: Ref<DownloadTask[]>;
  isTaskOperating?: (taskId: number) => boolean;
  isBulkOperating?: Ref<boolean>;
}

export interface TaskToolbarBatchResult {
  successCount: number;
  failureCount: number;
}

const createEnabledCategories: MainNavCategory[] = ["all", "downloading", "completed"];

export function useTaskToolbar({
  activeCategory,
  isRuntimeExiting,
  visibleTasks,
  clearTrashTasks,
  isTaskOperating = () => false,
  isBulkOperating,
}: UseTaskToolbarOptions) {
  const pauseCandidates = computed(() =>
    (visibleTasks?.value ?? []).filter(
      (task) => (task.status === "active" || task.status === "pending") && !isTaskOperating(task.id),
    ),
  );
  const resumeCandidates = computed(() =>
    (visibleTasks?.value ?? []).filter(
      (task) =>
        !task.confirmationRequired &&
        (task.status === "paused" || task.status === "error") &&
        !isTaskOperating(task.id),
    ),
  );
  const deleteCandidates = computed(() =>
    activeCategory.value === "trash" || activeCategory.value === "extensions"
      ? []
      : (visibleTasks?.value ?? []).filter((task) => task.status !== "removed" && !isTaskOperating(task.id)),
  );
  const clearTrashCandidates = computed(() =>
    activeCategory.value === "trash"
      ? (clearTrashTasks?.value ?? visibleTasks?.value ?? []).filter(
          (task) => task.status === "removed" && !isTaskOperating(task.id),
        )
      : [],
  );
  const isBusy = computed(() => Boolean(isBulkOperating?.value));
  const canCreate = computed(
    () => !isRuntimeExiting.value && createEnabledCategories.includes(activeCategory.value),
  );
  const canRefresh = computed(
    () => !isRuntimeExiting.value && activeCategory.value !== "extensions",
  );
  const canPauseVisible = computed(
    () => !isRuntimeExiting.value && !isBusy.value && pauseCandidates.value.length > 0,
  );
  const canResumeVisible = computed(
    () => !isRuntimeExiting.value && !isBusy.value && resumeCandidates.value.length > 0,
  );
  const canDeleteVisible = computed(
    () => !isRuntimeExiting.value && !isBusy.value && deleteCandidates.value.length > 0,
  );
  const canClearTrash = computed(
    () => !isRuntimeExiting.value && !isBusy.value && clearTrashCandidates.value.length > 0,
  );

  return {
    canCreate,
    canRefresh,
    canPauseVisible,
    canResumeVisible,
    canDeleteVisible,
    canClearTrash,
    pauseCandidates,
    resumeCandidates,
    deleteCandidates,
    clearTrashCandidates,
  };
}

export async function runTaskToolbarBatch(
  tasks: DownloadTask[],
  operation: (task: DownloadTask) => Promise<unknown>,
): Promise<TaskToolbarBatchResult> {
  let successCount = 0;
  let failureCount = 0;

  for (const task of tasks) {
    try {
      await operation(task);
      successCount += 1;
    } catch {
      failureCount += 1;
    }
  }

  return { successCount, failureCount };
}
