<script setup lang="ts">
import AboutDialog from "../features/about/components/AboutDialog.vue";
import DiagnosticsDialog from "../features/diagnostics/components/DiagnosticsDialog.vue";
import HelpDialog from "../features/help/components/HelpDialog.vue";
import SettingsDialog from "../features/settings/components/SettingsDialog.vue";
import TaskCreateDialog from "../features/tasks/components/TaskCreateDialog.vue";
import type { AppInfo, AppUpdateCheck, BackendPing } from "../types/app";
import type { Aria2ProcessStatus, Aria2RpcStatus } from "../types/aria2";

type Aria2StatusSnapshot = {
  process: Aria2ProcessStatus;
  rpc: Aria2RpcStatus;
};

const props = defineProps<{
  appInfo: AppInfo | null;
  backendPing: BackendPing | null;
  showCreateDialog: boolean;
  showAbout: boolean;
  showSettings: boolean;
  showHelp: boolean;
  showDiagnostics: boolean;
  updateCheck: AppUpdateCheck | null;
  isCheckingUpdate: boolean;
  aria2Process: Aria2ProcessStatus | null;
  aria2Rpc: Aria2RpcStatus | null;
}>();

const emit = defineEmits<{
  "update:showCreateDialog": [value: boolean];
  "update:showAbout": [value: boolean];
  "update:showSettings": [value: boolean];
  "update:showHelp": [value: boolean];
  "update:showDiagnostics": [value: boolean];
  openSettings: [];
  openRpcGuide: [];
  taskCreated: [];
  checkUpdate: [];
  refreshStatus: [];
  engineStatusUpdated: [status: Aria2StatusSnapshot];
}>();
</script>

<template>
  <TaskCreateDialog
    :show="props.showCreateDialog"
    @update:show="emit('update:showCreateDialog', $event)"
    @created="emit('taskCreated')"
  />
  <AboutDialog
    :show="props.showAbout"
    :app-info="props.appInfo"
    :update-check="props.updateCheck"
    :is-checking-update="props.isCheckingUpdate"
    @update:show="emit('update:showAbout', $event)"
    @check-update="emit('checkUpdate')"
    @open-settings="emit('openSettings')"
  />
  <SettingsDialog
    :show="props.showSettings"
    @update:show="emit('update:showSettings', $event)"
    @open-rpc-guide="emit('openRpcGuide')"
  />
  <HelpDialog
    :show="props.showHelp"
    @update:show="emit('update:showHelp', $event)"
  />
  <DiagnosticsDialog
    :show="props.showDiagnostics"
    :app-info="props.appInfo"
    :backend-ping="props.backendPing"
    :aria2-process="props.aria2Process"
    :aria2-rpc="props.aria2Rpc"
    @update:show="emit('update:showDiagnostics', $event)"
    @refresh-status="emit('refreshStatus')"
    @engine-status-updated="emit('engineStatusUpdated', $event)"
  />
</template>
