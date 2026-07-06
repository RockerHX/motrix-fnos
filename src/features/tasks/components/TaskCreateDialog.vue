<script setup lang="ts">
import { computed, toRef } from "vue";
import {
  NAlert,
  NButton,
  NCard,
  NCollapse,
  NCollapseItem,
  NForm,
  NFormItem,
  NGi,
  NGrid,
  NInput,
  NModal,
  NSelect,
  NSpace,
  NTabPane,
  NTabs,
} from "naive-ui";
import { useI18n } from "../../../i18n";
import { useMobileLayout } from "../../../app/composables/useMobileLayout";
import { useTaskCreateForm } from "../composables/useTaskCreateForm";

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  created: [];
}>();

const { t } = useI18n();
const { isMobileLayout } = useMobileLayout();
const advancedGridCols = computed(() => (isMobileLayout.value ? 1 : 2));
const {
  taskStore,
  form,
  activeInputType,
  formErrorMessage,
  accessiblePaths,
  isLoadingAccessiblePaths,
  accessiblePathsError,
  urlFeedback,
  urlValidationStatus,
  accessiblePathOptions,
  canSubmit,
  isMaskClosable,
  submitCreateTask,
  closeDialog,
} = useTaskCreateForm({
  show: toRef(props, "show"),
  onClose: () => emit("update:show", false),
  onCreated: () => emit("created"),
});
</script>

<template>
  <NModal :show="show" :mask-closable="isMaskClosable" @update:show="(nextShow: boolean) => !nextShow && closeDialog()">
    <NCard class="task-create-card app-dialog" role="dialog" aria-modal="true">
      <template #header>
        <div>
          <p class="task-create-eyebrow app-dialog-eyebrow">{{ t("create.eyebrow") }}</p>
          <h2>{{ t("create.title") }}</h2>
        </div>
      </template>
      <template #header-extra>
        <NButton
          quaternary
          circle
          :title="t('common.close')"
          :aria-label="t('common.close')"
          :disabled="taskStore.isCreating || taskStore.isRuntimeExiting"
          @click="closeDialog"
        >
          ×
        </NButton>
      </template>

      <NForm class="task-create-form" label-placement="top" @submit.prevent="submitCreateTask">
        <NTabs v-model:value="activeInputType" class="task-create-tabs" type="segment" animated>
          <NTabPane name="url" :tab="t('create.tab.url')" />
          <NTabPane name="batch" :tab="t('create.tab.batch')" disabled />
          <NTabPane name="torrent" :tab="t('create.tab.torrent')" disabled />
          <NTabPane name="magnet" :tab="t('create.tab.magnet')" disabled />
        </NTabs>

        <NFormItem :label="t('create.url.label')" :feedback="urlFeedback" :validation-status="urlValidationStatus">
          <NInput v-model:value="form.url" type="text" placeholder="https://example.com/file.zip" />
        </NFormItem>

        <NFormItem :label="t('create.fileName.label')">
          <NInput v-model:value="form.fileName" :placeholder="t('create.fileName.placeholder')" />
        </NFormItem>

        <NFormItem :label="t('create.saveDir.label')">
          <NSpace vertical class="full-width">
            <NSelect
              v-model:value="form.saveDir"
              :options="accessiblePathOptions"
              :loading="isLoadingAccessiblePaths"
              :disabled="isLoadingAccessiblePaths || accessiblePaths.length === 0"
              filterable
              :placeholder="t('create.saveDir.placeholder')"
            />
            <span class="field-hint">{{ t("create.saveDir.hint") }}</span>
            <NAlert v-if="accessiblePathsError" type="error" class="inline-alert">
              {{ t("create.saveDir.loadFailed", { message: accessiblePathsError }) }}
            </NAlert>
            <NAlert v-else-if="!isLoadingAccessiblePaths && accessiblePaths.length === 0" type="warning" class="inline-alert">
              {{ t("create.saveDir.empty") }}
            </NAlert>
          </NSpace>
        </NFormItem>

        <NFormItem :label="t('create.startMode.label')">
          <NTabs v-model:value="form.startMode" class="start-mode-tabs" type="segment">
            <NTabPane name="now" :tab="t('create.startMode.now')" />
            <NTabPane name="paused" :tab="t('create.startMode.paused')" />
          </NTabs>
        </NFormItem>

        <NFormItem :label="t('create.note.label')">
          <NInput v-model:value="form.note" :placeholder="t('create.note.placeholder')" />
        </NFormItem>

        <NCollapse>
          <NCollapseItem :title="t('create.advanced')" name="advanced">
            <NGrid :cols="advancedGridCols" :x-gap="12" :y-gap="12">
              <NGi><NInput :placeholder="t('create.advanced.category')" disabled /></NGi>
              <NGi><NInput :placeholder="t('create.advanced.connections')" disabled /></NGi>
              <NGi><NInput :placeholder="t('create.advanced.speedLimit')" disabled /></NGi>
              <NGi><NInput :placeholder="t('create.advanced.proxy')" disabled /></NGi>
            </NGrid>
          </NCollapseItem>
        </NCollapse>

        <NAlert v-if="formErrorMessage" type="error" class="form-alert">{{ formErrorMessage }}</NAlert>

        <NSpace justify="end" class="dialog-actions">
          <NButton :disabled="taskStore.isCreating || taskStore.isRuntimeExiting" @click="closeDialog">{{ t("common.cancel") }}</NButton>
          <NButton type="primary" attr-type="submit" :loading="taskStore.isCreating" :disabled="!canSubmit">{{ t("create.submit") }}</NButton>
        </NSpace>
      </NForm>
    </NCard>
  </NModal>
</template>

<style scoped>
.task-create-card {
  --app-dialog-width: 720px;
  min-width: 0;
  overscroll-behavior: contain;
}

.task-create-eyebrow {
  --app-dialog-eyebrow-color: var(--app-text-accent-soft);
  --app-dialog-eyebrow-weight: 700;
  --app-dialog-eyebrow-letter-spacing: 0.08em;
}

h2 {
  margin: 0;
  font-size: 22px;
}

.full-width {
  width: 100%;
}

.field-hint {
  color: #83958e;
  font-size: 12px;
  line-height: 1.5;
}

.inline-alert {
  width: 100%;
  word-break: break-word;
}

.form-alert {
  margin-top: 16px;
  word-break: break-word;
}

.dialog-actions {
  margin-top: 22px;
}

@media (max-width: 767px) {
  .task-create-form {
    width: 100%;
    padding-bottom: 4px;
    scroll-padding-bottom: calc(88px + var(--app-safe-area-bottom));
  }

  .task-create-form :deep(.n-form-item-label) {
    padding-bottom: 8px;
  }

  .task-create-form :deep(.n-form-item-blank),
  .task-create-form :deep(.n-base-selection),
  .task-create-form :deep(.n-input),
  .task-create-tabs,
  .start-mode-tabs {
    width: 100%;
    min-width: 0;
  }

  .task-create-form :deep(.n-input__input-el),
  .task-create-form :deep(.n-base-selection-label),
  .task-create-form :deep(.n-base-selection-input__content) {
    font-size: 16px;
  }

  .dialog-actions {
    position: sticky;
    bottom: 0;
    z-index: 1;
    margin-top: 18px;
    padding-top: 14px;
    padding-bottom: calc(8px + var(--app-safe-area-bottom));
    background: linear-gradient(180deg, rgba(27, 31, 29, 0), rgba(27, 31, 29, 0.94) 18px, #1b1f1d 100%);
  }
}
</style>
