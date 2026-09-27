import { computed, ref, watch, type Ref } from "vue";
import type { TranslationKey } from "../../../i18n";
import type { MainNavCategory } from "../../../types/navigation";
import type { DownloadTask } from "../../../types/tasks";

export interface TaskCategoryEmptyState {
  title: string;
  description: string;
  titleKey: TranslationKey;
  descriptionKey: TranslationKey;
  showCreateAction: boolean;
  showSettingsAction: boolean;
}

interface UseTaskCategoryViewOptions {
  tasks: Ref<DownloadTask[]>;
  removedTasks: Ref<DownloadTask[]>;
  isRuntimeExiting: Ref<boolean>;
  isMobileLayout: Ref<boolean>;
  initialCategory?: MainNavCategory;
}

export type TaskStatusFilter = "all" | "paused" | "error";

const emptyStateByCategory: Record<MainNavCategory, TaskCategoryEmptyState> = {
  all: {
    title: "",
    description: "",
    titleKey: "empty.all.title",
    descriptionKey: "empty.all.description",
    showCreateAction: true,
    showSettingsAction: true,
  },
  downloading: {
    title: "",
    description: "",
    titleKey: "empty.downloading.title",
    descriptionKey: "empty.downloading.description",
    showCreateAction: true,
    showSettingsAction: true,
  },
  completed: {
    title: "",
    description: "",
    titleKey: "empty.completed.title",
    descriptionKey: "empty.completed.description",
    showCreateAction: false,
    showSettingsAction: false,
  },
  trash: {
    title: "",
    description: "",
    titleKey: "empty.trash.title",
    descriptionKey: "empty.trash.description",
    showCreateAction: false,
    showSettingsAction: false,
  },
  extensions: {
    title: "",
    description: "",
    titleKey: "empty.extensions.title",
    descriptionKey: "empty.extensions.description",
    showCreateAction: false,
    showSettingsAction: false,
  },
};

export function useTaskCategoryView({
  tasks,
  removedTasks,
  isRuntimeExiting,
  isMobileLayout,
  initialCategory = "all",
}: UseTaskCategoryViewOptions) {
  const activeCategory = ref<MainNavCategory>(initialCategory);
  const taskStatusFilter = ref<TaskStatusFilter>("all");
  const visibleTasks = computed(() =>
    filterTasksByCategory(tasks.value, removedTasks.value, activeCategory.value, taskStatusFilter.value),
  );
  const isExtensionsCategory = computed(() => activeCategory.value === "extensions");
  const hasVisibleTasks = computed(() => visibleTasks.value.length > 0);
  const contentViewKey = computed(() =>
    `${activeCategory.value}-${isExtensionsCategory.value ? "extensions" : hasVisibleTasks.value ? "list" : "empty"}`,
  );
  const emptyState = computed(() => {
    if (activeCategory.value === "all" && taskStatusFilter.value === "paused") {
      return {
        ...emptyStateByCategory.all,
        titleKey: "empty.paused.title" as TranslationKey,
        descriptionKey: "empty.paused.description" as TranslationKey,
      };
    }
    if (activeCategory.value === "all" && taskStatusFilter.value === "error") {
      return {
        ...emptyStateByCategory.all,
        titleKey: "empty.error.title" as TranslationKey,
        descriptionKey: "empty.error.description" as TranslationKey,
      };
    }
    return emptyStateByCategory[activeCategory.value];
  });
  const showFloatingAdd = computed(() => {
    if (isRuntimeExiting.value) {
      return false;
    }

    if (!["all", "downloading", "completed"].includes(activeCategory.value)) {
      return false;
    }

    if (isMobileLayout.value && !hasVisibleTasks.value && emptyState.value.showCreateAction) {
      return false;
    }

    return true;
  });

  watch(activeCategory, (category) => {
    if (category !== "all") taskStatusFilter.value = "all";
  });

  return {
    activeCategory,
    taskStatusFilter,
    visibleTasks,
    isExtensionsCategory,
    hasVisibleTasks,
    contentViewKey,
    emptyState,
    showFloatingAdd,
  };
}

function filterTasksByCategory(
  tasks: DownloadTask[],
  removedTasks: DownloadTask[],
  category: MainNavCategory,
  statusFilter: TaskStatusFilter,
) {
  switch (category) {
    case "all":
      return statusFilter === "all" ? tasks : tasks.filter((task) => task.status === statusFilter);
    case "downloading":
      return tasks.filter(
        (task) => task.confirmationRequired || task.status === "pending" || task.status === "active",
      );
    case "completed":
      return tasks.filter((task) => task.status === "complete");
    case "extensions":
      return [];
    case "trash":
      return removedTasks;
  }
}
