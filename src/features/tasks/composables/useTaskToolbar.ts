import { computed, type Ref } from "vue";
import type { MainNavCategory } from "../../../types/navigation";

interface UseTaskToolbarOptions {
  activeCategory: Ref<MainNavCategory>;
  isRuntimeExiting: Ref<boolean>;
}

const createEnabledCategories: MainNavCategory[] = ["downloading", "completed", "stopped"];

export function useTaskToolbar({ activeCategory, isRuntimeExiting }: UseTaskToolbarOptions) {
  const canCreate = computed(
    () => !isRuntimeExiting.value && createEnabledCategories.includes(activeCategory.value),
  );
  const canRefresh = computed(
    () => !isRuntimeExiting.value && activeCategory.value !== "extensions",
  );

  return {
    canCreate,
    canRefresh,
  };
}
