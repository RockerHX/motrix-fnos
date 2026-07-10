<script setup lang="ts">
import { NButton, NCard, NModal, NSpace } from "naive-ui";
import { useI18n } from "../../../i18n";

const props = withDefaults(
  defineProps<{
    show: boolean;
    taskCount: number;
    isLoading?: boolean;
  }>(),
  {
    isLoading: false,
  },
);

const emit = defineEmits<{
  "update:show": [show: boolean];
  confirm: [];
}>();

const { t } = useI18n();

function close() {
  if (!props.isLoading) {
    emit("update:show", false);
  }
}

function confirm() {
  if (!props.isLoading) {
    emit("confirm");
  }
}
</script>

<template>
  <NModal :show="props.show" :mask-closable="!props.isLoading" @update:show="emit('update:show', $event)">
    <NCard class="bulk-delete-card app-dialog" role="dialog" aria-modal="true" :title="t('task.bulk.deleteTitle')">
      <p class="bulk-delete-description">
        {{ t("task.bulk.deleteConfirm", { count: props.taskCount }) }}
      </p>

      <template #footer>
        <NSpace justify="end">
          <NButton :disabled="props.isLoading" @click="close">{{ t("common.cancel") }}</NButton>
          <NButton type="error" :loading="props.isLoading" @click="confirm">
            {{ t("task.actions.delete") }}
          </NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>
</template>

<style scoped>
.bulk-delete-card {
  --app-dialog-width: 520px;
  --app-dialog-mobile-margin: 16px;
}

.bulk-delete-description {
  margin: 0;
  color: var(--app-text-secondary);
  line-height: 1.6;
}
</style>
