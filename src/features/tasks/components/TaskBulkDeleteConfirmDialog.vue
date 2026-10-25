<script setup lang="ts">
import { NCheckbox } from "naive-ui";
import { ref, watch } from "vue";
import AppConfirmDialog from "../../../components/ui/AppConfirmDialog.vue";
import { useI18n } from "../../../i18n";

const props = withDefaults(
  defineProps<{
    show: boolean;
    taskCount: number;
    canDeleteFiles: boolean;
    isLoading?: boolean;
    mode?: "delete" | "clearTrash";
  }>(),
  {
    isLoading: false,
    mode: "delete",
  },
);

const emit = defineEmits<{
  "update:show": [show: boolean];
  confirm: [deleteFiles: boolean];
}>();

const { t } = useI18n();
const deleteFiles = ref(false);
watch(() => [props.show, props.mode, props.canDeleteFiles], () => {
  deleteFiles.value = false;
});

function updateShow(show: boolean) {
  if (!props.isLoading) {
    emit("update:show", show);
  }
}

function confirm() {
  if (!props.isLoading) {
    emit("confirm", props.mode === "clearTrash" && props.canDeleteFiles && deleteFiles.value);
  }
}
</script>

<template>
  <AppConfirmDialog
    :show="props.show"
    :title="t(props.mode === 'clearTrash' ? 'task.bulk.clearTrashTitle' : 'task.bulk.deleteTitle')"
    :mask-closable="!props.isLoading"
    :loading="props.isLoading"
    confirm-type="error"
    width="520px"
    @update:show="updateShow"
    @confirm="confirm"
  >
    <p class="bulk-delete-description">
      {{ t(props.mode === "clearTrash" ? "task.bulk.clearTrashConfirm" : "task.bulk.deleteConfirm", { count: props.taskCount }) }}
    </p>

    <template #extra>
      <NCheckbox v-if="props.mode === 'clearTrash' && props.canDeleteFiles" v-model:checked="deleteFiles" :disabled="props.isLoading">{{ t("task.delete.files") }}</NCheckbox>
    </template>
    <template #confirm-label>{{ t(props.mode === "clearTrash" ? "task.bulk.clearTrash" : "task.actions.delete") }}</template>
  </AppConfirmDialog>
</template>

<style scoped src="./TaskBulkDeleteConfirmDialog.css"></style>
