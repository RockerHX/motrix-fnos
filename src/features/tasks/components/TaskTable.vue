<script setup lang="ts">
import { NPagination } from "naive-ui";
import { useMobileLayout } from "../../../app/composables/useMobileLayout";
import { TASK_PAGE_SIZES } from "../composables/useTaskPagination";
import { useI18n } from "../../../i18n";
import TaskMobileList from "./TaskMobileList.vue";
import TaskDesktopList from "./TaskDesktopList.vue";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  tasks: DownloadTask[];
  page: number;
  pageSize: number;
  itemCount: number;
  showPagination: boolean;
}>();

const emit = defineEmits<{
  "update:page": [page: number];
  "update:pageSize": [pageSize: number];
}>();

const { isMobileLayout } = useMobileLayout();
const { t } = useI18n();
</script>

<template>
  <section class="task-table">
    <div class="task-table-list">
      <TaskMobileList v-if="isMobileLayout" :tasks="props.tasks" />
      <TaskDesktopList v-else :tasks="props.tasks" />
    </div>
    <footer v-if="props.showPagination" class="task-pagination" :aria-label="t('task.pagination.label')">
      <NPagination
        :page="props.page"
        :page-size="props.pageSize"
        :item-count="props.itemCount"
        :page-sizes="[...TASK_PAGE_SIZES]"
        :simple="isMobileLayout"
        show-size-picker
        @update:page="emit('update:page', $event)"
        @update:page-size="emit('update:pageSize', $event)"
      />
    </footer>
  </section>
</template>

<style scoped>
.task-table {
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
  display: grid;
  grid-template-rows: minmax(0, 1fr) auto;
  overflow: hidden;
}

.task-table-list {
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.task-pagination {
  display: flex;
  justify-content: flex-end;
  border-top: 1px solid var(--app-color-border-subtle);
  padding: 10px 16px;
  background: var(--app-color-surface);
}

@media (max-width: 767px) {
  .task-pagination {
    justify-content: center;
    padding: 8px var(--app-mobile-page-gutter) calc(8px + var(--app-safe-area-bottom));
  }
}
</style>
