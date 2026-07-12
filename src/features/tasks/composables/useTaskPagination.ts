import { computed, ref, watch, type Ref } from "vue";
import type { MainNavCategory } from "../../../types/navigation";
import type { DownloadTask } from "../../../types/tasks";

export const TASK_PAGE_SIZES = [20, 50, 100] as const;
export const DEFAULT_TASK_PAGE_SIZE = TASK_PAGE_SIZES[0];

interface UseTaskPaginationOptions {
  tasks: Ref<DownloadTask[]>;
  activeCategory: Ref<MainNavCategory>;
}

export function useTaskPagination({ tasks, activeCategory }: UseTaskPaginationOptions) {
  const page = ref(1);
  const pageSize = ref<number>(DEFAULT_TASK_PAGE_SIZE);
  const itemCount = computed(() => tasks.value.length);
  const pageCount = computed(() => Math.max(1, Math.ceil(itemCount.value / pageSize.value)));
  const pagedTasks = computed(() => {
    const start = (page.value - 1) * pageSize.value;
    return tasks.value.slice(start, start + pageSize.value);
  });
  const showPagination = computed(() => itemCount.value > DEFAULT_TASK_PAGE_SIZE);

  watch(activeCategory, () => resetPage());
  watch(pageSize, () => resetPage());
  watch(pageCount, (nextPageCount) => {
    if (page.value > nextPageCount) {
      page.value = nextPageCount;
    }
  });

  function resetPage() {
    page.value = 1;
  }

  return {
    page,
    pageSize,
    itemCount,
    pagedTasks,
    showPagination,
    resetPage,
  };
}
