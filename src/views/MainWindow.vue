<script setup lang="ts">
import { storeToRefs } from "pinia";
import { computed, onMounted, ref, watch } from "vue";
import { useMessage } from "naive-ui";
import { useMobileLayout } from "../app/composables/useMobileLayout";
import DiagnosticsDialog from "../features/diagnostics/components/DiagnosticsDialog.vue";
import ExtensionsPlaceholder from "../features/extensions/components/ExtensionsPlaceholder.vue";
import HelpDialog from "../features/help/components/HelpDialog.vue";
import SettingsDialog from "../features/settings/components/SettingsDialog.vue";
import TaskCreateDialog from "../features/tasks/components/TaskCreateDialog.vue";
import TaskEmptyState from "../features/tasks/components/TaskEmptyState.vue";
import TaskTable from "../features/tasks/components/TaskTable.vue";
import { useTaskStore } from "../features/tasks/stores/taskStore";
import AppShell from "../layouts/AppShell.vue";
import { getAria2ProcessStatus, pingAria2Rpc } from "../services/aria2";
import { useI18n, type TranslationKey } from "../i18n";
import type { AppInfo, BackendPing } from "../types/app";
import type { Aria2ProcessStatus, Aria2RpcStatus } from "../types/aria2";
import type { MainNavCategory } from "../types/navigation";
import type { DownloadTask } from "../types/tasks";

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
const showDiagnostics = ref(false);
const showHelp = ref(false);
const showSettings = ref(false);
const activeCategory = ref<MainNavCategory>("downloading");
const visibleTasks = computed(() => filterTasksByCategory(tasks.value, activeCategory.value));
const emptyState = computed(() => emptyStateByCategory[activeCategory.value]);
const showFloatingAdd = computed(() => {
  if (!["downloading", "completed", "stopped"].includes(activeCategory.value)) {
    return false;
  }

  if (isMobileLayout.value && visibleTasks.value.length === 0 && emptyState.value.showCreateAction) {
    return false;
  }

  return true;
});

const emptyStateByCategory: Record<
  MainNavCategory,
  {
    title: string;
    description: string;
    titleKey: TranslationKey;
    descriptionKey: TranslationKey;
    showCreateAction: boolean;
    showSettingsAction: boolean;
  }
> = {
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
  showCreateDialog.value = true;
}

function selectCategory(category: MainNavCategory) {
  activeCategory.value = category;
  if (category === "trash") {
    void refreshRemovedTasks(true);
  }
}

async function handleTaskCreated() {
  message.success(t("task.created"));
  void refreshPhaseStatus();
}

async function refreshTasks(showError = false) {
  const result = await taskStore.refreshTasks({ showError });
  if (result.refreshError) {
    message.error(result.refreshError);
  }
  flushTaskErrorMessages();
}

async function refreshRemovedTasks(showError = false) {
  const result = await taskStore.refreshRemovedTasks({ showError });
  if (result.refreshError) {
    message.error(result.refreshError);
  }
}

function flushTaskErrorMessages() {
  for (const errorMessage of taskStore.consumeTaskErrorMessages()) {
    message.error(errorMessage);
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
  () => taskStore.pendingTaskErrorMessages.length,
  (count) => {
    if (count > 0) {
      flushTaskErrorMessages();
    }
  },
);

onMounted(() => {
  void refreshPhaseStatus();
  void refreshTasks(true);
});

function filterTasksByCategory(nextTasks: DownloadTask[], category: MainNavCategory) {
  switch (category) {
    case "downloading":
      return nextTasks.filter((task) => task.status === "pending" || task.status === "active");
    case "completed":
      return nextTasks.filter((task) => task.status === "complete");
    case "stopped":
      return nextTasks.filter((task) => task.status === "paused" || task.status === "error");
    case "extensions":
      return [];
    case "trash":
      return removedTasks.value;
  }
}
</script>

<template>
  <AppShell
    :app-info="appInfo"
    :active-category="activeCategory"
    @open-diagnostics="showDiagnostics = true"
    @open-help="showHelp = true"
    @open-settings="showSettings = true"
    @select-category="selectCategory"
  >
    <ExtensionsPlaceholder v-if="activeCategory === 'extensions'" />
    <template v-else>
      <TaskEmptyState
        v-if="visibleTasks.length === 0"
        :title="t(emptyState.titleKey)"
        :description="t(emptyState.descriptionKey)"
        :show-create-action="emptyState.showCreateAction"
        :show-settings-action="emptyState.showSettingsAction"
        @create="openCreateDialog"
        @open-settings="showSettings = true"
      />
      <TaskTable v-else :tasks="visibleTasks" />
    </template>

    <template #overlay>
      <button
        v-if="showFloatingAdd"
        type="button"
        class="floating-add"
        :aria-label="t('empty.create')"
        @click="openCreateDialog"
      >
        ＋
      </button>

      <TaskCreateDialog v-model:show="showCreateDialog" @created="handleTaskCreated" />
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
