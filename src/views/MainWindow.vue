<script setup lang="ts">
import { storeToRefs } from "pinia";
import { computed, onMounted, ref, watch } from "vue";
import { useMessage } from "naive-ui";
import DiagnosticsDialog from "../features/diagnostics/components/DiagnosticsDialog.vue";
import SettingsDialog from "../features/settings/components/SettingsDialog.vue";
import TaskCreateDialog from "../features/tasks/components/TaskCreateDialog.vue";
import TaskEmptyState from "../features/tasks/components/TaskEmptyState.vue";
import TaskTable from "../features/tasks/components/TaskTable.vue";
import { useTaskStore } from "../features/tasks/stores/taskStore";
import AppShell from "../layouts/AppShell.vue";
import { getAria2ProcessStatus, pingAria2Rpc } from "../services/aria2";
import type { AppInfo, BackendPing } from "../types/app";
import type { Aria2ProcessStatus, Aria2RpcStatus } from "../types/aria2";
import type { MainNavCategory } from "../types/navigation";
import type { DownloadTask } from "../types/tasks";

const props = defineProps<{
  appInfo: AppInfo | null;
  backendPing: BackendPing | null;
  errorMessage: string;
}>();

const message = useMessage();
const taskStore = useTaskStore();
const { tasks, removedTasks } = storeToRefs(taskStore);
const aria2Process = ref<Aria2ProcessStatus | null>(null);
const aria2Rpc = ref<Aria2RpcStatus | null>(null);
const showCreateDialog = ref(false);
const showDiagnostics = ref(false);
const showSettings = ref(false);
const activeCategory = ref<MainNavCategory>("downloading");
const visibleTasks = computed(() => filterTasksByCategory(tasks.value, activeCategory.value));
const emptyState = computed(() => emptyStateByCategory[activeCategory.value]);
const showFloatingAdd = computed(() =>
  ["downloading", "completed", "stopped"].includes(activeCategory.value),
);

const emptyStateByCategory: Record<
  MainNavCategory,
  {
    title: string;
    description: string;
    showCreateAction: boolean;
    showSettingsAction: boolean;
  }
> = {
  downloading: {
    title: "暂无下载中任务",
    description: "点击添加任务，或粘贴 HTTP / HTTPS 链接开始下载。",
    showCreateAction: true,
    showSettingsAction: true,
  },
  completed: {
    title: "暂无已完成任务",
    description: "任务下载完成后会显示在这里。",
    showCreateAction: false,
    showSettingsAction: false,
  },
  stopped: {
    title: "暂无已停止任务",
    description: "暂停或下载失败的任务会显示在这里，可从列表中继续处理。",
    showCreateAction: false,
    showSettingsAction: false,
  },
  trash: {
    title: "回收站暂无任务",
    description: "删除后的任务记录会显示在这里。",
    showCreateAction: false,
    showSettingsAction: false,
  },
  extensions: {
    title: "暂无扩展",
    description: "扩展页面将在后续步骤提供说明。",
    showCreateAction: false,
    showSettingsAction: false,
  },
};

async function refreshPhaseStatus() {
  const [process, rpc] = await Promise.all([getAria2ProcessStatus(), pingAria2Rpc()]);
  aria2Process.value = process;
  aria2Rpc.value = rpc;
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
  message.success("任务已添加");
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
    @open-settings="showSettings = true"
    @select-category="selectCategory"
  >
    <TaskEmptyState
      v-if="visibleTasks.length === 0"
      :title="emptyState.title"
      :description="emptyState.description"
      :show-create-action="emptyState.showCreateAction"
      :show-settings-action="emptyState.showSettingsAction"
      @create="openCreateDialog"
      @open-settings="showSettings = true"
    />
    <TaskTable v-else :tasks="visibleTasks" />

    <template #overlay>
      <button
        v-if="showFloatingAdd"
        type="button"
        class="floating-add"
        aria-label="添加任务"
        @click="openCreateDialog"
      >
        ＋
      </button>

      <TaskCreateDialog v-model:show="showCreateDialog" @created="handleTaskCreated" />
      <SettingsDialog v-model:show="showSettings" />
      <DiagnosticsDialog
        v-model:show="showDiagnostics"
        :app-info="appInfo"
        :backend-ping="backendPing"
        :aria2-process="aria2Process"
        :aria2-rpc="aria2Rpc"
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
}
</style>
