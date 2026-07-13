<script setup lang="ts">
import { NCheckbox } from "naive-ui";
import { ref, watch } from "vue";
import AppConfirmDialog from "../../../components/ui/AppConfirmDialog.vue";
import type { TaskActionConfirmTexts, TaskActionLabels, TaskActionState } from "./taskActionViewModel";

const props = defineProps<{
  show: boolean;
  state: TaskActionState;
  labels: TaskActionLabels;
  confirmTexts: TaskActionConfirmTexts;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  confirm: [deleteFiles: boolean];
}>();

const deleteFiles = ref(false);

watch(
  () => props.show,
  (show) => {
    if (show) {
      deleteFiles.value = false;
    }
  },
);
</script>

<template>
  <AppConfirmDialog
    :show="show"
    :title="confirmTexts.deleteTitle"
    :mask-closable="!state.isOperating"
    :loading="state.isOperating"
    :disabled="state.isActionDisabled"
    :confirm-text="confirmTexts.deleteConfirmText"
    confirm-type="error"
    @update:show="emit('update:show', $event)"
    @confirm="emit('confirm', deleteFiles)"
  >
    <template #extra>
      <NCheckbox v-model:checked="deleteFiles">{{ confirmTexts.deleteFilesLabel }}</NCheckbox>
    </template>
    <template #confirm-label>{{ labels.delete }}</template>
  </AppConfirmDialog>
</template>
