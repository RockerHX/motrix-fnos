<script setup lang="ts">
import { NCheckbox } from "naive-ui";
import { ref, watch } from "vue";
import AppConfirmDialog from "../../../components/ui/AppConfirmDialog.vue";
import type { TaskActionConfirmTexts, TaskActionLabels, TaskActionState } from "./taskActionViewModel";

const props = defineProps<{
  show: boolean;
  canDeleteFiles: boolean;
  state: TaskActionState;
  labels: TaskActionLabels;
  confirmTexts: TaskActionConfirmTexts;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  confirm: [deleteFiles: boolean];
}>();
const deleteFiles = ref(false);
watch(() => [props.show, props.canDeleteFiles], () => {
  deleteFiles.value = false;
});
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
    @confirm="emit('confirm', canDeleteFiles && deleteFiles)"
  >
    <template #extra>
      <NCheckbox v-if="canDeleteFiles" v-model:checked="deleteFiles" :disabled="state.isActionDisabled">{{ confirmTexts.deleteFilesLabel }}</NCheckbox>
    </template>
    <template #confirm-label>{{ labels.permanentDelete }}</template>
  </AppConfirmDialog>
</template>
