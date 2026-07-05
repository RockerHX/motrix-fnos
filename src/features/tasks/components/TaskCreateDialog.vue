<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
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
  useMessage,
} from "naive-ui";
import { getAccessiblePaths } from "../../../services/storage";
import { useI18n } from "../../../i18n";
import { useSettingsStore } from "../../settings/stores/settingsStore";
import { useTaskStore } from "../stores/taskStore";

const LAST_SAVE_DIR_KEY = "motrix-fnos:last-save-dir";

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  created: [];
}>();

const taskStore = useTaskStore();
const settingsStore = useSettingsStore();
const message = useMessage();
const { t } = useI18n();

const form = reactive({
  url: "",
  fileName: "",
  saveDir: "",
  startMode: "now",
  note: "",
});
const activeInputType = ref("url");
const formErrorMessage = ref("");
const accessiblePaths = ref<string[]>([]);
const isLoadingAccessiblePaths = ref(false);
const accessiblePathsError = ref("");

const isUrlValid = computed(() => /^https?:\/\/.+/i.test(form.url.trim()));
const urlFeedback = computed(() => (form.url && !isUrlValid.value ? t("create.url.invalid") : undefined));
const urlValidationStatus = computed(() => (form.url && !isUrlValid.value ? "error" : undefined));
const accessiblePathOptions = computed(() =>
  accessiblePaths.value.map((path) => ({
    label: path,
    value: path,
  })),
);
const canSubmit = computed(
  () =>
    isUrlValid.value &&
    !!form.saveDir &&
    !taskStore.isCreating &&
    !taskStore.isRuntimeExiting &&
    !isLoadingAccessiblePaths.value,
);

watch(
  () => props.show,
  (show) => {
    if (show) {
      formErrorMessage.value = "";
      void refreshAccessiblePaths();
    }
  },
);

onMounted(() => {
  void refreshAccessiblePaths();
});

async function submitCreateTask() {
  if (taskStore.isRuntimeExiting) {
    message.warning(t("task.runtimeExiting"));
    return;
  }
  if (!isUrlValid.value) {
    formErrorMessage.value = t("create.url.required");
    return;
  }
  if (!form.saveDir) {
    formErrorMessage.value = t("create.saveDir.required");
    return;
  }

  formErrorMessage.value = "";

  try {
    await taskStore.createTask({
      url: form.url,
      fileName: form.fileName || null,
      saveDir: form.saveDir,
    });
    rememberSaveDir(form.saveDir);
    resetForm();
    emit("update:show", false);
    emit("created");
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

function closeDialog() {
  if (taskStore.isCreating || taskStore.isRuntimeExiting) {
    return;
  }

  emit("update:show", false);
}

function resetForm() {
  form.url = "";
  form.fileName = "";
  form.saveDir = "";
  form.startMode = "now";
  form.note = "";
  activeInputType.value = "url";
  formErrorMessage.value = "";
}

async function refreshAccessiblePaths() {
  isLoadingAccessiblePaths.value = true;
  accessiblePathsError.value = "";

  try {
    const [response, config] = await Promise.all([getAccessiblePaths(), settingsStore.loadConfig()]);
    accessiblePaths.value = response.paths;
    syncSelectedSaveDir(config.defaultDownloadDir);
  } catch (error) {
    accessiblePaths.value = [];
    form.saveDir = "";
    accessiblePathsError.value = getErrorMessage(error);
  } finally {
    isLoadingAccessiblePaths.value = false;
  }
}

function syncSelectedSaveDir(defaultDownloadDir: string) {
  if (form.saveDir && accessiblePaths.value.includes(form.saveDir)) {
    return;
  }

  const remembered = readRememberedSaveDir();
  if (defaultDownloadDir && accessiblePaths.value.includes(defaultDownloadDir)) {
    form.saveDir = defaultDownloadDir;
    return;
  }
  if (remembered && accessiblePaths.value.includes(remembered)) {
    form.saveDir = remembered;
    return;
  }
  form.saveDir = accessiblePaths.value[0] || "";
}

function rememberSaveDir(path: string) {
  localStorage.setItem(LAST_SAVE_DIR_KEY, path);
}

function readRememberedSaveDir() {
  return localStorage.getItem(LAST_SAVE_DIR_KEY) || "";
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  const message = String(error);
  return message || t("task.operationFailed");
}
</script>

<template>
  <NModal :show="show" :mask-closable="!taskStore.isCreating && !taskStore.isRuntimeExiting" @update:show="(nextShow: boolean) => !nextShow && closeDialog()">
    <NCard class="task-create-card" role="dialog" aria-modal="true">
      <template #header>
        <div>
          <p class="eyebrow">{{ t("create.eyebrow") }}</p>
          <h2>{{ t("create.title") }}</h2>
        </div>
      </template>
      <template #header-extra>
        <NButton quaternary circle :disabled="taskStore.isCreating || taskStore.isRuntimeExiting" @click="closeDialog">×</NButton>
      </template>

      <NForm @submit.prevent="submitCreateTask">
        <NTabs v-model:value="activeInputType" type="segment" animated>
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
          <NTabs v-model:value="form.startMode" type="segment">
            <NTabPane name="now" :tab="t('create.startMode.now')" />
            <NTabPane name="paused" :tab="t('create.startMode.paused')" />
          </NTabs>
        </NFormItem>

        <NFormItem :label="t('create.note.label')">
          <NInput v-model:value="form.note" :placeholder="t('create.note.placeholder')" />
        </NFormItem>

        <NCollapse>
          <NCollapseItem :title="t('create.advanced')" name="advanced">
            <NGrid :cols="2" :x-gap="12" :y-gap="12">
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
  width: min(720px, calc(100vw - 48px));
  max-height: calc(var(--app-viewport-height) - 48px);
  overflow: auto;
}

.eyebrow {
  margin: 0 0 6px;
  color: #67dca0;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
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
}

.form-alert {
  margin-top: 16px;
}

.dialog-actions {
  margin-top: 22px;
}

@media (max-width: 767px) {
  .task-create-card {
    width: calc(100vw - 16px);
    max-height: calc(var(--app-viewport-height) - 16px);
    border-radius: 18px;
  }
}
</style>
