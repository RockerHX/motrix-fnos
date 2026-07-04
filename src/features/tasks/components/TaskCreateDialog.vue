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

const form = reactive({
  url: "",
  fileName: "",
  saveDir: "",
  startMode: "now",
  note: "",
});
const activeInputType = ref("URL 下载");
const formErrorMessage = ref("");
const accessiblePaths = ref<string[]>([]);
const isLoadingAccessiblePaths = ref(false);
const accessiblePathsError = ref("");

const isUrlValid = computed(() => /^https?:\/\/.+/i.test(form.url.trim()));
const urlFeedback = computed(() => (form.url && !isUrlValid.value ? "当前仅支持 HTTP / HTTPS 链接" : undefined));
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
    message.warning("应用正在退出，请稍候");
    return;
  }
  if (!isUrlValid.value) {
    formErrorMessage.value = "请输入有效的 HTTP / HTTPS 下载链接";
    return;
  }
  if (!form.saveDir) {
    formErrorMessage.value = "请选择已授权的保存目录";
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
  activeInputType.value = "URL 下载";
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
  return message || "操作失败，请稍后重试";
}
</script>

<template>
  <NModal :show="show" :mask-closable="!taskStore.isCreating && !taskStore.isRuntimeExiting" @update:show="(nextShow: boolean) => !nextShow && closeDialog()">
    <NCard class="task-create-card" role="dialog" aria-modal="true">
      <template #header>
        <div>
          <p class="eyebrow">New Task</p>
          <h2>新建下载任务</h2>
        </div>
      </template>
      <template #header-extra>
        <NButton quaternary circle :disabled="taskStore.isCreating || taskStore.isRuntimeExiting" @click="closeDialog">×</NButton>
      </template>

      <NForm @submit.prevent="submitCreateTask">
        <NTabs v-model:value="activeInputType" type="segment" animated>
          <NTabPane name="URL 下载" tab="URL 下载" />
          <NTabPane name="批量 URL" tab="批量 URL" disabled />
          <NTabPane name="种子文件（后期）" tab="种子文件（后期）" disabled />
          <NTabPane name="磁力链接（后期）" tab="磁力链接（后期）" disabled />
        </NTabs>

        <NFormItem label="下载链接" :feedback="urlFeedback" :validation-status="urlValidationStatus">
          <NInput v-model:value="form.url" type="text" placeholder="https://example.com/file.zip" />
        </NFormItem>

        <NFormItem label="文件名">
          <NInput v-model:value="form.fileName" placeholder="留空则从链接自动识别" />
        </NFormItem>

        <NFormItem label="保存路径">
          <NSpace vertical class="full-width">
            <NSelect
              v-model:value="form.saveDir"
              :options="accessiblePathOptions"
              :loading="isLoadingAccessiblePaths"
              :disabled="isLoadingAccessiblePaths || accessiblePaths.length === 0"
              filterable
              placeholder="请选择已授权的保存目录"
            />
            <span class="field-hint">目录来自飞牛应用设置中的文件夹授权；如刚修改授权，请重新打开新建任务或刷新页面。</span>
            <NAlert v-if="accessiblePathsError" type="error" class="inline-alert">
              读取授权目录失败：{{ accessiblePathsError }}
            </NAlert>
            <NAlert v-else-if="!isLoadingAccessiblePaths && accessiblePaths.length === 0" type="warning" class="inline-alert">
              未检测到已授权目录，请先在飞牛应用设置中为 Motrix 添加读写文件夹授权，然后重新打开新建任务。
            </NAlert>
          </NSpace>
        </NFormItem>

        <NFormItem label="开始方式">
          <NTabs v-model:value="form.startMode" type="segment">
            <NTabPane name="now" tab="立即开始" />
            <NTabPane name="paused" tab="添加后暂停" />
          </NTabs>
        </NFormItem>

        <NFormItem label="备注">
          <NInput v-model:value="form.note" placeholder="可选" />
        </NFormItem>

        <NCollapse>
          <NCollapseItem title="高级设置" name="advanced">
            <NGrid :cols="2" :x-gap="12" :y-gap="12">
              <NGi><NInput placeholder="分类：默认" disabled /></NGi>
              <NGi><NInput placeholder="连接数：16" disabled /></NGi>
              <NGi><NInput placeholder="限速：不限速" disabled /></NGi>
              <NGi><NInput placeholder="代理：后期支持" disabled /></NGi>
            </NGrid>
          </NCollapseItem>
        </NCollapse>

        <NAlert v-if="formErrorMessage" type="error" class="form-alert">{{ formErrorMessage }}</NAlert>

        <NSpace justify="end" class="dialog-actions">
          <NButton :disabled="taskStore.isCreating || taskStore.isRuntimeExiting" @click="closeDialog">取消</NButton>
          <NButton type="primary" attr-type="submit" :loading="taskStore.isCreating" :disabled="!canSubmit">开始下载</NButton>
        </NSpace>
      </NForm>
    </NCard>
  </NModal>
</template>

<style scoped>
.task-create-card {
  width: min(720px, calc(100vw - 48px));
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
</style>
