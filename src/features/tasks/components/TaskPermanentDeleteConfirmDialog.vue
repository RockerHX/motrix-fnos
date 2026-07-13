<script setup lang="ts">
import AppConfirmDialog from "../../../components/ui/AppConfirmDialog.vue";
import type { TaskActionConfirmTexts, TaskActionLabels, TaskActionState } from "./taskActionViewModel";

defineProps<{
  show: boolean;
  state: TaskActionState;
  labels: TaskActionLabels;
  confirmTexts: TaskActionConfirmTexts;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  confirm: [];
}>();
</script>

<template>
  <AppConfirmDialog
    :show="show"
    :title="confirmTexts.permanentDeleteTitle"
    :mask-closable="!state.isOperating"
    :loading="state.isOperating"
    :disabled="state.isActionDisabled"
    :confirm-text="confirmTexts.permanentDeleteConfirmText"
    confirm-type="error"
    @update:show="emit('update:show', $event)"
    @confirm="emit('confirm')"
  >
    <template #confirm-label>{{ labels.permanentDelete }}</template>
  </AppConfirmDialog>
</template>
