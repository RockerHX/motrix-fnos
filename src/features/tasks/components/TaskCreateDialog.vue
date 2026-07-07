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
  NInputNumber,
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
  batchFailedItems,
  accessiblePaths,
  isLoadingAccessiblePaths,
  accessiblePathsError,
  urlFeedback,
  urlValidationStatus,
  magnetFeedback,
  magnetValidationStatus,
  accessiblePathOptions,
  canSubmit,
  isMaskClosable,
  selectTorrentFile,
  submitCreateTask,
  closeDialog,
} = useTaskCreateForm({
  show: toRef(props, "show"),
  onClose: () => emit("update:show", false),
  onCreated: () => emit("created"),
});

const selectedTorrentFileName = computed(() => form.torrentFile?.name || t("create.torrent.notSelected"));

function handleTorrentFileChange(event: Event) {
  const input = event.target as HTMLInputElement;
  selectTorrentFile(input.files?.[0] ?? null);
}
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
          <NTabPane name="batch" :tab="t('create.tab.batch')" />
          <NTabPane name="torrent" :tab="t('create.tab.torrent')" />
          <NTabPane name="magnet" :tab="t('create.tab.magnet')" />
        </NTabs>

        <template v-if="activeInputType === 'url'">
          <NFormItem :label="t('create.url.label')" :feedback="urlFeedback" :validation-status="urlValidationStatus">
            <NInput v-model:value="form.url" type="text" placeholder="https://example.com/file.zip" />
          </NFormItem>

          <NFormItem :label="t('create.fileName.label')">
            <NInput v-model:value="form.fileName" :placeholder="t('create.fileName.placeholder')" />
          </NFormItem>
        </template>

        <NFormItem v-else-if="activeInputType === 'batch'" :label="t('create.batch.label')">
          <NInput
            v-model:value="form.batchUrls"
            type="textarea"
            :autosize="{ minRows: 5, maxRows: 10 }"
            :placeholder="t('create.batch.placeholder')"
          />
        </NFormItem>

        <NFormItem v-else-if="activeInputType === 'torrent'" :label="t('create.torrent.label')">
          <NSpace vertical class="full-width">
            <input class="torrent-file-input" type="file" accept=".torrent,application/x-bittorrent" @change="handleTorrentFileChange" />
            <span class="field-hint">{{ selectedTorrentFileName }}</span>
          </NSpace>
        </NFormItem>

        <NFormItem v-else :label="t('create.magnet.label')" :feedback="magnetFeedback" :validation-status="magnetValidationStatus">
          <NInput v-model:value="form.magnet" type="text" placeholder="magnet:?xt=urn:btih:..." />
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

        <NCollapse>
          <NCollapseItem :title="t('create.advanced')" name="advanced">
            <NGrid :cols="advancedGridCols" :x-gap="12" :y-gap="12">
              <NGi>
                <NFormItem :label="t('create.advanced.category.label')" path="category">
                  <NInput v-model:value="form.category" :placeholder="t('create.advanced.category.placeholder')" />
                </NFormItem>
              </NGi>
              <NGi>
                <NFormItem :label="t('create.advanced.connections.label')" path="connections">
                  <NInputNumber v-model:value="form.connections" class="full-width" :min="1" :max="64" :precision="0" />
                </NFormItem>
              </NGi>
              <NGi>
                <NFormItem :label="t('create.advanced.speedLimit.label')" path="downloadLimitKb">
                  <NInputNumber v-model:value="form.downloadLimitKb" class="full-width" :min="0" :precision="0" />
                </NFormItem>
              </NGi>
              <NGi>
                <NFormItem :label="t('create.advanced.proxy.label')" path="proxy">
                  <NInput v-model:value="form.proxy" :placeholder="t('create.advanced.proxy.placeholder')" />
                </NFormItem>
              </NGi>
            </NGrid>
            <p class="field-hint advanced-hint">{{ t("create.advanced.hint") }}</p>
          </NCollapseItem>
        </NCollapse>

        <NAlert v-if="formErrorMessage" type="error" class="form-alert">{{ formErrorMessage }}</NAlert>
        <NAlert v-if="batchFailedItems.length > 0" type="warning" class="form-alert">
          <p class="batch-failure-title">{{ t("create.batch.failedTitle") }}</p>
          <ul class="batch-failure-list">
            <li v-for="item in batchFailedItems" :key="`${item.input}-${item.message}`">
              {{ item.input }}：{{ item.message }}
            </li>
          </ul>
        </NAlert>

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
  color: var(--app-text-dim);
  font-size: 12px;
  line-height: 1.5;
}

.torrent-file-input {
  max-width: 100%;
  color: var(--app-text);
}

.advanced-hint {
  margin: 8px 0 0;
}

.inline-alert {
  width: 100%;
  word-break: break-word;
}

.form-alert {
  margin-top: 16px;
  word-break: break-word;
}

.batch-failure-title {
  margin: 0 0 8px;
  font-weight: 600;
}

.batch-failure-list {
  margin: 0;
  padding-left: 18px;
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
  .task-create-form :deep(.n-input-number),
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
