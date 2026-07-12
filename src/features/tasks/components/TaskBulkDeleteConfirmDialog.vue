<script setup lang="ts">
import AppConfirmDialog from "../../../components/ui/AppConfirmDialog.vue";
import { useI18n } from "../../../i18n";

const props = withDefaults(
  defineProps<{
    show: boolean;
    taskCount: number;
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
  confirm: [];
}>();

const { t } = useI18n();

function updateShow(show: boolean) {
  if (!props.isLoading) {
    emit("update:show", show);
  }
}

function confirm() {
  if (!props.isLoading) {
    emit("confirm");
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

    <template #confirm-label>{{ t(props.mode === "clearTrash" ? "task.bulk.clearTrash" : "task.actions.delete") }}</template>
  </AppConfirmDialog>
</template>

<style scoped>
.bulk-delete-description {
  margin: 0;
  color: var(--app-text-secondary);
  line-height: 1.6;
}
</style>
