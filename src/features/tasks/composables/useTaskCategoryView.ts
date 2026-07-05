import { computed, ref, type Ref } from "vue";
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

const emptyStateByCategory: Record<MainNavCategory, TaskCategoryEmptyState> = {
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
  stopped: {
    title: "",
    description: "",
    titleKey: "empty.stopped.title",
    descriptionKey: "empty.stopped.description",
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
  initialCategory = "downloading",
}: UseTaskCategoryViewOptions) {
  const activeCategory = ref<MainNavCategory>(initialCategory);
  const visibleTasks = computed(() => filterTasksByCategory(tasks.value, removedTasks.value, activeCategory.value));
  const isExtensionsCategory = computed(() => activeCategory.value === "extensions");
  const hasVisibleTasks = computed(() => visibleTasks.value.length > 0);
  const contentViewKey = computed(() =>
    `${activeCategory.value}-${isExtensionsCategory.value ? "extensions" : hasVisibleTasks.value ? "list" : "empty"}`,
  );
  const emptyState = computed(() => emptyStateByCategory[activeCategory.value]);
  const showFloatingAdd = computed(() => {
    if (isRuntimeExiting.value) {
      return false;
    }

    if (!["downloading", "completed", "stopped"].includes(activeCategory.value)) {
      return false;
    }

    if (isMobileLayout.value && !hasVisibleTasks.value && emptyState.value.showCreateAction) {
      return false;
    }

    return true;
  });

  return {
    activeCategory,
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
) {
  switch (category) {
    case "downloading":
      return tasks.filter((task) => task.status === "pending" || task.status === "active");
    case "completed":
      return tasks.filter((task) => task.status === "complete");
    case "stopped":
      return tasks.filter((task) => task.status === "paused" || task.status === "error");
    case "extensions":
      return [];
    case "trash":
      return removedTasks;
  }
}
