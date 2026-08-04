<script setup lang="ts">
import { NSpace } from "naive-ui";
import AppConfirmDialog from "../../../components/ui/AppConfirmDialog.vue";
import type { TaskActionConfirmTexts, TaskActionLabels, TaskActionState } from "./taskActionViewModel";
import TaskProxyToggle from "./TaskProxyToggle.vue";

defineProps<{
  show: boolean;
  state: TaskActionState;
  labels: TaskActionLabels;
  confirmTexts: TaskActionConfirmTexts;
  useProxy: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  "update:useProxy": [enabled: boolean];
  confirm: [useProxy: boolean];
}>();
</script>

<template>
  <AppConfirmDialog
    :show="show"
    :title="confirmTexts.restoreTitle"
    :mask-closable="!state.isOperating"
    :loading="state.isOperating"
    :disabled="state.isActionDisabled"
    confirm-type="primary"
    @update:show="emit('update:show', $event)"
    @confirm="emit('confirm', useProxy)"
  >
    <NSpace vertical>
      <p>{{ confirmTexts.restoreConfirmText }}</p>
      <TaskProxyToggle
        :value="useProxy"
        :disabled="state.isActionDisabled"
        :loading="state.isOperating"
        @update:value="emit('update:useProxy', $event)"
      />
    </NSpace>
    <template #confirm-label>{{ labels.restore }}</template>
  </AppConfirmDialog>
</template>
