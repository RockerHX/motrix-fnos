<script setup lang="ts">
import { storeToRefs } from "pinia";
import { computed, onMounted, ref, watch } from "vue";
import { useMessage } from "naive-ui";
import { useMobileLayout } from "../app/composables/useMobileLayout";
import AboutDialog from "../features/about/components/AboutDialog.vue";
import { checkAppUpdate } from "../features/about/services/aboutService";
import DiagnosticsDialog from "../features/diagnostics/components/DiagnosticsDialog.vue";
import ExtensionsPlaceholder from "../features/extensions/components/ExtensionsPlaceholder.vue";
import HelpDialog from "../features/help/components/HelpDialog.vue";
import SettingsDialog from "../features/settings/components/SettingsDialog.vue";
import TaskCreateDialog from "../features/tasks/components/TaskCreateDialog.vue";
import TaskEmptyState from "../features/tasks/components/TaskEmptyState.vue";
import TaskTable from "../features/tasks/components/TaskTable.vue";
import { useTaskCategoryView } from "../features/tasks/composables/useTaskCategoryView";
import { useTaskToasts } from "../features/tasks/composables/useTaskToasts";
import { useTaskStore } from "../features/tasks/stores/taskStore";
import AppShell from "../layouts/AppShell.vue";
import { getAria2ProcessStatus, pingAria2Rpc } from "../services/aria2";
import { useI18n } from "../i18n";
import type { AppInfo, AppUpdateCheck, BackendPing } from "../types/app";
import type { Aria2ProcessStatus, Aria2RpcStatus } from "../types/aria2";
import type { MainNavCategory } from "../types/navigation";

type Aria2StatusSnapshot = {
  process: Aria2ProcessStatus;
  rpc: Aria2RpcStatus;
};

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
const aria2Process = ref<Aria2ProcessStatus | null>(null);
const aria2Rpc = ref<Aria2RpcStatus | null>(null);
const showCreateDialog = ref(false);
const showAbout = ref(false);
const showDiagnostics = ref(false);
const showHelp = ref(false);
const showSettings = ref(false);
const updateCheck = ref<AppUpdateCheck | null>(null);
const isCheckingUpdate = ref(false);
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

async function refreshPhaseStatus() {
  const [process, rpc] = await Promise.all([getAria2ProcessStatus(), pingAria2Rpc()]);
  aria2Process.value = process;
  aria2Rpc.value = rpc;
}

function updateAria2Status(status: Aria2StatusSnapshot) {
  aria2Process.value = status.process;
  aria2Rpc.value = status.rpc;
}

function openCreateDialog() {
  if (taskStore.isRuntimeExiting) {
    message.warning(t("task.runtimeExiting"));
    return;
  }

  showCreateDialog.value = true;
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
  void refreshPhaseStatus();
}

async function handleCheckUpdate() {
  isCheckingUpdate.value = true;
  try {
    updateCheck.value = await checkAppUpdate();
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    message.error(errorMessage || t("task.operationFailed"));
  } finally {
    isCheckingUpdate.value = false;
  }
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
  void refreshPhaseStatus();
  void refreshTasks(true);
});
</script>

<template>
  <AppShell
    :app-info="appInfo"
    :active-category="activeCategory"
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

      <TaskCreateDialog v-model:show="showCreateDialog" @created="handleTaskCreated" />
      <AboutDialog
        v-model:show="showAbout"
        :app-info="props.appInfo"
        :update-check="updateCheck"
        :is-checking-update="isCheckingUpdate"
        @check-update="handleCheckUpdate"
      />
      <SettingsDialog v-model:show="showSettings" />
      <HelpDialog v-model:show="showHelp" />
      <DiagnosticsDialog
        v-model:show="showDiagnostics"
        :app-info="appInfo"
        :backend-ping="backendPing"
        :aria2-process="aria2Process"
        :aria2-rpc="aria2Rpc"
        @refresh-status="refreshPhaseStatus"
        @engine-status-updated="updateAria2Status"
      />
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
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
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
