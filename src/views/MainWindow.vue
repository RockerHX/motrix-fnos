<script setup lang="ts">
import { storeToRefs } from "pinia";
import { computed, ref, toRef, watch } from "vue";
import { useMessage } from "naive-ui";
import { useMobileLayout } from "../app/composables/useMobileLayout";
import AppIcon from "../components/AppIcon.vue";
import { useUpdateCheck } from "../features/about/composables/useUpdateCheck";
import { useAria2Status } from "../features/diagnostics/composables/useAria2Status";
import ExtensionsPlaceholder from "../features/extensions/components/ExtensionsPlaceholder.vue";
import TaskBulkDeleteConfirmDialog from "../features/tasks/components/TaskBulkDeleteConfirmDialog.vue";
import TaskEmptyState from "../features/tasks/components/TaskEmptyState.vue";
import TaskFileConfirmCoordinator from "../features/tasks/components/TaskFileConfirmCoordinator.vue";
import TaskTable from "../features/tasks/components/TaskTable.vue";
import { useTaskBulkActions } from "../features/tasks/composables/useTaskBulkActions";
import { useTaskCategoryView } from "../features/tasks/composables/useTaskCategoryView";
import { useTaskPagination } from "../features/tasks/composables/useTaskPagination";
import { useTaskToasts } from "../features/tasks/composables/useTaskToasts";
import { useTaskTopbarActions } from "../features/tasks/composables/useTaskTopbarActions";
import { useTaskToolbar } from "../features/tasks/composables/useTaskToolbar";
import { useTaskStore } from "../features/tasks/stores/taskStore";
import AppShell from "../layouts/AppShell.vue";
import MainWindowDialogs from "./MainWindowDialogs.vue";
import { useMainWindowDialogs } from "./composables/useMainWindowDialogs";
import { useMainWindowLifecycle } from "./composables/useMainWindowLifecycle";
import { useI18n } from "../i18n";
import type { AppInfo, BackendPing } from "../types/app";
import type { MainNavCategory } from "../types/navigation";
const props = defineProps<{
  appInfo: AppInfo | null;
  backendPing: BackendPing | null;
  errorMessage: string;
}>();

const message = useMessage();
const { t } = useI18n();
const { isMobileLayout } = useMobileLayout();
const taskStore = useTaskStore();
const { tasks, removedTasks } = storeToRefs(taskStore);
const isToolbarBulkOperating = ref(false);
const { aria2Process, aria2Rpc, refreshAria2Status, updateAria2Status } = useAria2Status();
const { updateCheck, isCheckingUpdate, runUpdateCheck } = useUpdateCheck({
  message,
  fallbackMessage: t("task.operationFailed"),
});
const {
  activeCategory,
  visibleTasks,
  isExtensionsCategory,
  hasVisibleTasks,
  contentViewKey,
  emptyState,
  showFloatingAdd,
} = useTaskCategoryView({
  tasks,
  removedTasks,
  isRuntimeExiting: computed(() => taskStore.isRuntimeExiting),
  isMobileLayout,
});
const { refreshTasks, refreshRemovedTasks } = useTaskToasts({
  taskStore,
  message,
});
const pagination = useTaskPagination({ tasks: visibleTasks, activeCategory });
const toolbar = useTaskToolbar({
  activeCategory,
  visibleTasks: pagination.pagedTasks,
  clearTrashTasks: removedTasks,
  isRuntimeExiting: computed(() => taskStore.isRuntimeExiting),
  isBulkOperating: isToolbarBulkOperating,
  isTaskOperating: taskStore.isTaskOperating,
});
const dialogs = useMainWindowDialogs({ taskStore, toolbar, message, t });
const bulkActions = useTaskBulkActions({ taskStore, toolbar, message, t });
isToolbarBulkOperating.value = bulkActions.isBulkOperating.value;
watch(bulkActions.isBulkOperating, (value) => (isToolbarBulkOperating.value = value));
const topbar = useTaskTopbarActions({
  activeCategory,
  taskStore,
  toolbar,
  refreshTasks,
  refreshRemovedTasks,
  refreshAria2Status,
  t,
});
useMainWindowLifecycle({
  errorMessage: toRef(props, "errorMessage"),
  isRuntimeExiting: computed(() => taskStore.isRuntimeExiting),
  showCreateDialog: dialogs.showCreateDialog,
  message,
  refreshTasks,
  refreshAria2Status,
});

function selectCategory(category: MainNavCategory) {
  const previousCategory = activeCategory.value;
  if (previousCategory === category) {
    return;
  }

  activeCategory.value = category;
  if (previousCategory !== "trash" && category === "trash") {
    void refreshRemovedTasks(true);
  }
}

async function handleTaskCreated() {
  pagination.resetPage();
  message.success(t("task.created"));
  void refreshAria2Status();
}

</script>

<template>
  <AppShell
    :app-info="appInfo"
    :active-category="activeCategory"
    :topbar-actions="topbar.topbarActions.value"
    @create="dialogs.handleToolbarCreate"
    @refresh="topbar.refresh"
    @pause-visible="bulkActions.pauseVisibleTasks"
    @resume-visible="bulkActions.resumeVisibleTasks"
    @delete-visible="bulkActions.requestDeleteVisibleTasks"
    @clear-trash="bulkActions.requestClearTrash"
    @open-about="dialogs.showAbout.value = true"
    @open-diagnostics="dialogs.showDiagnostics.value = true"
    @open-help="dialogs.showHelp.value = true"
    @open-settings="dialogs.showSettings.value = true"
    @select-category="selectCategory"
  >
    <ExtensionsPlaceholder v-if="isExtensionsCategory" :key="contentViewKey" />
    <template v-else>
      <TaskEmptyState
        v-if="!hasVisibleTasks"
        :key="contentViewKey"
        :title="t(emptyState.titleKey)"
        :description="t(emptyState.descriptionKey)"
        :show-create-action="emptyState.showCreateAction"
        :disable-create-action="taskStore.isRuntimeExiting"
        :show-settings-action="emptyState.showSettingsAction"
        @create="dialogs.openCreateDialog"
        @open-settings="dialogs.showSettings.value = true"
      />
      <TaskTable
        v-else
        :key="contentViewKey"
        :tasks="pagination.pagedTasks.value"
        :page="pagination.page.value"
        :page-size="pagination.pageSize.value"
        :item-count="pagination.itemCount.value"
        :show-pagination="pagination.showPagination.value"
        @update:page="pagination.page.value = $event"
        @update:page-size="pagination.pageSize.value = $event"
      />
    </template>

    <template #overlay>
      <button
        v-if="isMobileLayout && showFloatingAdd"
        type="button"
        class="floating-add"
        :title="t('empty.create')"
        :aria-label="t('empty.create')"
        @click="dialogs.openCreateDialog"
      >
        <AppIcon name="plus" :size="28" />
      </button>

      <TaskBulkDeleteConfirmDialog
        :show="bulkActions.showBulkDeleteConfirm.value"
        :task-count="bulkActions.bulkDeleteTaskCount.value"
        :is-loading="bulkActions.isBulkOperating.value"
        :mode="bulkActions.bulkDeleteMode.value"
        @update:show="bulkActions.showBulkDeleteConfirm.value = $event"
        @confirm="bulkActions.confirmDeleteVisibleTasks"
      />

      <MainWindowDialogs
        :app-info="props.appInfo"
        :backend-ping="backendPing"
        :show-create-dialog="dialogs.showCreateDialog.value"
        :show-about="dialogs.showAbout.value"
        :show-settings="dialogs.showSettings.value"
        :show-help="dialogs.showHelp.value"
        :show-diagnostics="dialogs.showDiagnostics.value"
        :update-check="updateCheck"
        :is-checking-update="isCheckingUpdate"
        :aria2-process="aria2Process"
        :aria2-rpc="aria2Rpc"
        @update:show-create-dialog="dialogs.showCreateDialog.value = $event"
        @update:show-about="dialogs.showAbout.value = $event"
        @update:show-settings="dialogs.showSettings.value = $event"
        @update:show-help="dialogs.showHelp.value = $event"
        @update:show-diagnostics="dialogs.showDiagnostics.value = $event"
        @task-created="handleTaskCreated"
        @check-update="runUpdateCheck"
        @refresh-status="refreshAria2Status"
        @engine-status-updated="updateAria2Status"
      />
      <TaskFileConfirmCoordinator />
    </template>
  </AppShell>
</template>

<style scoped>
.floating-add {
  position: absolute;
  right: 26px;
  bottom: 24px;
  width: 52px;
  height: 52px;
  border: 0;
  border-radius: 999px;
  color: #101710;
  background: #68ae5a;
  font: inherit;
  font-size: 30px;
  line-height: 1;
  box-shadow: var(--app-shadow-floating);
  cursor: pointer;
  z-index: 2;
}

@media (max-width: 767px) {
  .floating-add {
    right: var(--app-mobile-fab-offset-right);
    bottom: calc(72px + var(--app-mobile-fab-offset-bottom));
    width: 56px;
    height: 56px;
    font-size: 32px;
  }
}
</style>
