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
  NUpload,
  NTabs,
} from "naive-ui";
import type { UploadFileInfo, UploadOnChange, UploadOnRemove } from "naive-ui";
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
const torrentUploadFileList = computed<UploadFileInfo[]>(() => {
  if (!form.torrentFile) {
    return [];
  }

  return [
    {
      id: `${form.torrentFile.name}-${form.torrentFile.lastModified}`,
      name: form.torrentFile.name,
      status: "pending",
      file: form.torrentFile,
      type: form.torrentFile.type,
    },
  ];
});

const handleTorrentUploadChange: UploadOnChange = ({ file }) => {
  selectTorrentFile(file.file ?? null);
};

const handleTorrentUploadRemove: UploadOnRemove = () => {
  selectTorrentFile(null);
  return true;
};
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
        <div class="task-create-fields">
          <NTabs v-model:value="activeInputType" class="task-create-tabs" type="segment" animated>
            <NTabPane name="url" :tab="t('create.tab.url')" />
            <NTabPane name="torrent" :tab="t('create.tab.torrent')" />
            <NTabPane name="magnet" :tab="t('create.tab.magnet')" />
          </NTabs>

          <template v-if="activeInputType === 'url'">
            <NFormItem :label="t('create.url.label')" :feedback="urlFeedback" :validation-status="urlValidationStatus">
              <NInput
                v-model:value="form.urls"
                type="textarea"
                :autosize="{ minRows: 5, maxRows: 10 }"
                :placeholder="t('create.url.placeholder')"
              />
            </NFormItem>
          </template>

          <NFormItem v-else-if="activeInputType === 'torrent'" :label="t('create.torrent.label')">
            <NSpace vertical class="full-width">
              <NUpload
                :file-list="torrentUploadFileList"
                :default-upload="false"
                :max="1"
                accept=".torrent,application/x-bittorrent"
                @change="handleTorrentUploadChange"
                @remove="handleTorrentUploadRemove"
              >
                <NButton secondary>{{ t("create.torrent.label") }}</NButton>
              </NUpload>
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
        </div>

        <NSpace justify="end" class="dialog-actions">
          <NButton :disabled="taskStore.isCreating || taskStore.isRuntimeExiting" @click="closeDialog">{{ t("common.cancel") }}</NButton>
          <NButton type="primary" attr-type="submit" :loading="taskStore.isCreating" :disabled="!canSubmit">{{ t("create.submit") }}</NButton>
        </NSpace>
      </NForm>
    </NCard>
  </NModal>
</template>

<style scoped src="./TaskCreateDialog.css"></style>
