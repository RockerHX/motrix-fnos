<script setup lang="ts">
import { storeToRefs } from "pinia";
import { onMounted, ref, watch } from "vue";
import { useMessage } from "naive-ui";
import DiagnosticsDialog from "../features/diagnostics/components/DiagnosticsDialog.vue";
import TaskCreateDialog from "../features/tasks/components/TaskCreateDialog.vue";
import TaskEmptyState from "../features/tasks/components/TaskEmptyState.vue";
import TaskTable from "../features/tasks/components/TaskTable.vue";
import { useTaskPolling } from "../features/tasks/composables/useTaskPolling";
import { useTaskStore } from "../features/tasks/stores/taskStore";
import AppShell from "../layouts/AppShell.vue";
import { getAria2ProcessStatus, pingAria2Rpc } from "../services/aria2";
import type { AppInfo, BackendPing } from "../types/app";
import type { Aria2ProcessStatus, Aria2RpcStatus } from "../types/aria2";

const props = defineProps<{
  appInfo: AppInfo | null;
  backendPing: BackendPing | null;
  errorMessage: string;
}>();

const message = useMessage();
const taskStore = useTaskStore();
const { tasks } = storeToRefs(taskStore);
const aria2Process = ref<Aria2ProcessStatus | null>(null);
const aria2Rpc = ref<Aria2RpcStatus | null>(null);
const showCreateDialog = ref(false);
const showDiagnostics = ref(false);

const { refresh: refreshTasks } = useTaskPolling({
  onRefreshError: (errorMessage) => message.error(errorMessage),
  onTaskError: (errorMessage) => message.error(errorMessage),
});

async function refreshPhaseStatus() {
  const [process, rpc] = await Promise.all([getAria2ProcessStatus(), pingAria2Rpc()]);
  aria2Process.value = process;
  aria2Rpc.value = rpc;
}

function openCreateDialog() {
  showCreateDialog.value = true;
}

async function handleTaskCreated() {
  message.success("任务已添加");
  void refreshPhaseStatus();
  await refreshTasks();
}

watch(
  () => props.errorMessage,
  (nextMessage) => {
    if (nextMessage) {
      message.error(nextMessage);
    }
  },
);

onMounted(() => {
  void refreshPhaseStatus();
});
</script>

<template>
  <AppShell :app-info="appInfo" @open-diagnostics="showDiagnostics = true">
    <TaskEmptyState v-if="tasks.length === 0" @create="openCreateDialog" />
    <TaskTable v-else :tasks="tasks" />

    <template #overlay>
      <button type="button" class="floating-add" aria-label="添加任务" @click="openCreateDialog">＋</button>

      <TaskCreateDialog v-model:show="showCreateDialog" @created="handleTaskCreated" />
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
