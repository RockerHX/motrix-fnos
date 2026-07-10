<script setup lang="ts">
import { storeToRefs } from "pinia";
import { computed, onMounted, ref, watch } from "vue";
import { useMessage } from "naive-ui";
import { useMobileLayout } from "../app/composables/useMobileLayout";
import { useUpdateCheck } from "../features/about/composables/useUpdateCheck";
import { useAria2Status } from "../features/diagnostics/composables/useAria2Status";
import ExtensionsPlaceholder from "../features/extensions/components/ExtensionsPlaceholder.vue";
import TaskEmptyState from "../features/tasks/components/TaskEmptyState.vue";
import TaskFileConfirmCoordinator from "../features/tasks/components/TaskFileConfirmCoordinator.vue";
import TaskTable from "../features/tasks/components/TaskTable.vue";
import { useTaskCategoryView } from "../features/tasks/composables/useTaskCategoryView";
import { useTaskToasts } from "../features/tasks/composables/useTaskToasts";
import { runTaskToolbarBatch, useTaskToolbar } from "../features/tasks/composables/useTaskToolbar";
import { useTaskStore } from "../features/tasks/stores/taskStore";
import AppShell from "../layouts/AppShell.vue";
import MainWindowDialogs from "./MainWindowDialogs.vue";
import { useI18n } from "../i18n";
import type { AppInfo, BackendPing } from "../types/app";
import type { MainNavCategory } from "../types/navigation";
import type { TopbarActionStates } from "../types/topbar";
import type { DownloadTask } from "../types/tasks";
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
const showCreateDialog = ref(false);
const showAbout = ref(false);
const showDiagnostics = ref(false);
const showHelp = ref(false);
const showSettings = ref(false);
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
const toolbar = useTaskToolbar({
  activeCategory,
  visibleTasks,
  isRuntimeExiting: computed(() => taskStore.isRuntimeExiting),
  isBulkOperating: isToolbarBulkOperating,
  isTaskOperating: taskStore.isTaskOperating,
});
const topbarActions = computed<TopbarActionStates>(() => ({
  create: {
    disabled: !toolbar.canCreate.value,
    title: toolbar.canCreate.value ? t("topbar.create") : createDisabledTitle(),
  },
  refresh: {
    disabled: !toolbar.canRefresh.value,
    title: toolbar.canRefresh.value ? t("common.refresh") : refreshDisabledTitle(),
  },
  pauseVisible: {
    disabled: !toolbar.canPauseVisible.value,
    title: toolbar.canPauseVisible.value ? t("topbar.pauseVisible") : batchDisabledTitle("pause"),
  },
  resumeVisible: {
    disabled: !toolbar.canResumeVisible.value,
    title: toolbar.canResumeVisible.value ? t("topbar.resumeVisible") : batchDisabledTitle("resume"),
  },
  deleteVisible: {
    disabled: !toolbar.canDeleteVisible.value,
    title: toolbar.canDeleteVisible.value ? t("topbar.deleteVisible") : batchDisabledTitle("delete"),
  },
}));

function createDisabledTitle() {
  if (taskStore.isRuntimeExiting) {
    return t("topbar.disabled.runtimeExiting");
  }
  if (activeCategory.value === "extensions") {
    return t("topbar.disabled.extensions");
  }
  return t("topbar.create");
}

function refreshDisabledTitle() {
  if (taskStore.isRuntimeExiting) {
    return t("topbar.disabled.runtimeExiting");
  }
  if (activeCategory.value === "extensions") {
    return t("topbar.disabled.extensions");
  }
  return t("common.refresh");
}

function batchDisabledTitle(action: "pause" | "resume" | "delete") {
  if (taskStore.isRuntimeExiting) {
    return t("topbar.disabled.runtimeExiting");
  }
  if (activeCategory.value === "extensions") {
    return t("topbar.disabled.extensions");
  }
  if (action === "pause") {
    return t("topbar.disabled.noPauseable");
  }
  if (action === "resume") {
    return t("topbar.disabled.noResumable");
  }
  return t("topbar.disabled.noDeletable");
}

function openCreateDialog() {
  if (taskStore.isRuntimeExiting) {
    message.warning(t("task.runtimeExiting"));
    return;
  }

  showCreateDialog.value = true;
}

function handleToolbarCreate() {
  if (!toolbar.canCreate.value) {
    if (taskStore.isRuntimeExiting) {
      message.warning(t("task.runtimeExiting"));
    }
    return;
  }

  openCreateDialog();
}

async function handleToolbarRefresh() {
  if (!toolbar.canRefresh.value) {
    return;
  }

  void refreshAria2Status();
  if (activeCategory.value === "trash") {
    await refreshRemovedTasks(true);
    return;
  }

  await refreshTasks(true);
}

async function handlePauseVisibleTasks() {
  await runVisibleTaskBatch(
    toolbar.pauseCandidates.value,
    (task) => taskStore.pauseTask(task.id),
    "task.bulk.pauseSuccess",
    "task.bulk.noPauseable",
  );
}

async function handleResumeVisibleTasks() {
  await runVisibleTaskBatch(
    toolbar.resumeCandidates.value,
    (task) => taskStore.resumeTask(task.id),
    "task.bulk.resumeSuccess",
    "task.bulk.noResumable",
  );
}

async function runVisibleTaskBatch(
  candidates: DownloadTask[],
  operation: (task: DownloadTask) => Promise<unknown>,
  successKey: "task.bulk.pauseSuccess" | "task.bulk.resumeSuccess",
  emptyKey: "task.bulk.noPauseable" | "task.bulk.noResumable",
) {
  if (candidates.length === 0) {
    message.warning(t(emptyKey));
    return;
  }

  isToolbarBulkOperating.value = true;
  try {
    const result = await runTaskToolbarBatch(candidates, operation);
    if (result.successCount > 0) {
      message.success(t(successKey, { count: result.successCount }));
    }
    if (result.failureCount > 0) {
      message.error(t("task.bulk.partialFailed", { count: result.failureCount }));
    }
  } finally {
    isToolbarBulkOperating.value = false;
  }
}

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
  message.success(t("task.created"));
  void refreshAria2Status();
}

watch(
  () => props.errorMessage,
  (nextMessage) => {
    if (nextMessage) {
      message.error(nextMessage);
    }
  },
);

watch(
  () => taskStore.isRuntimeExiting,
  (isRuntimeExiting) => {
    if (isRuntimeExiting) {
      showCreateDialog.value = false;
    }
  },
);

onMounted(() => {
  void refreshAria2Status();
  void refreshTasks(true);
});
</script>

<template>
  <AppShell
    :app-info="appInfo"
    :active-category="activeCategory"
    :topbar-actions="topbarActions"
    @create="handleToolbarCreate"
    @refresh="handleToolbarRefresh"
    @pause-visible="handlePauseVisibleTasks"
    @resume-visible="handleResumeVisibleTasks"
    @open-about="showAbout = true"
    @open-diagnostics="showDiagnostics = true"
    @open-help="showHelp = true"
    @open-settings="showSettings = true"
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
        @create="openCreateDialog"
        @open-settings="showSettings = true"
      />
      <TaskTable v-else :key="contentViewKey" :tasks="visibleTasks" />
    </template>

    <template #overlay>
      <button
        v-if="showFloatingAdd"
        type="button"
        class="floating-add"
        :title="t('empty.create')"
        :aria-label="t('empty.create')"
        @click="openCreateDialog"
      >
        ＋
      </button>

      <MainWindowDialogs
        :app-info="props.appInfo"
        :backend-ping="backendPing"
        :show-create-dialog="showCreateDialog"
        :show-about="showAbout"
        :show-settings="showSettings"
        :show-help="showHelp"
        :show-diagnostics="showDiagnostics"
        :update-check="updateCheck"
        :is-checking-update="isCheckingUpdate"
        :aria2-process="aria2Process"
        :aria2-rpc="aria2Rpc"
        @update:show-create-dialog="showCreateDialog = $event"
        @update:show-about="showAbout = $event"
        @update:show-settings="showSettings = $event"
        @update:show-help="showHelp = $event"
        @update:show-diagnostics="showDiagnostics = $event"
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
