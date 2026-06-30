<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { useMessage } from "naive-ui";
import { selectDownloadDirectory } from "../services/taskService";
import { useTaskStore } from "../stores/taskStore";

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  created: [];
}>();

const taskStore = useTaskStore();
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

const isUrlValid = computed(() => /^https?:\/\/.+/i.test(form.url.trim()));
const urlFeedback = computed(() => (form.url && !isUrlValid.value ? "当前仅支持 HTTP / HTTPS 链接" : undefined));
const urlValidationStatus = computed(() => (form.url && !isUrlValid.value ? "error" : undefined));

watch(
  () => props.show,
  (show) => {
    if (show) {
      formErrorMessage.value = "";
    }
  },
);

async function selectSaveDir() {
  const selected = await selectDownloadDirectory();
  if (selected) {
    form.saveDir = selected;
  }
}

async function submitCreateTask() {
  if (!isUrlValid.value) {
    formErrorMessage.value = "请输入有效的 HTTP / HTTPS 下载链接";
    return;
  }

  formErrorMessage.value = "";

  try {
    await taskStore.createTask({
      url: form.url,
      fileName: form.fileName || null,
      saveDir: form.saveDir || null,
    });
    resetForm();
    emit("update:show", false);
    emit("created");
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

function closeDialog() {
  if (taskStore.isCreating) {
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

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  const message = String(error);
  return message || "操作失败，请稍后重试";
}
</script>

<template>
  <n-modal :show="show" :mask-closable="!taskStore.isCreating" @update:show="(nextShow: boolean) => !nextShow && closeDialog()">
    <n-card class="task-create-card" role="dialog" aria-modal="true">
      <template #header>
        <div>
          <p class="eyebrow">New Task</p>
          <h2>新建下载任务</h2>
        </div>
      </template>
      <template #header-extra>
        <n-button quaternary circle :disabled="taskStore.isCreating" @click="closeDialog">×</n-button>
      </template>

      <n-form @submit.prevent="submitCreateTask">
        <n-tabs v-model:value="activeInputType" type="segment" animated>
          <n-tab-pane name="URL 下载" tab="URL 下载" />
          <n-tab-pane name="批量 URL" tab="批量 URL" disabled />
          <n-tab-pane name="种子文件（后期）" tab="种子文件（后期）" disabled />
          <n-tab-pane name="磁力链接（后期）" tab="磁力链接（后期）" disabled />
        </n-tabs>

        <n-form-item label="下载链接" :feedback="urlFeedback" :validation-status="urlValidationStatus">
          <n-input v-model:value="form.url" type="text" placeholder="https://example.com/file.zip" />
        </n-form-item>

        <n-form-item label="文件名">
          <n-input v-model:value="form.fileName" placeholder="留空则从链接自动识别" />
        </n-form-item>

        <n-form-item label="保存路径">
          <n-space vertical class="full-width">
            <n-input v-model:value="form.saveDir" placeholder="留空使用 ~/Downloads，也可输入或选择目录" />
            <n-button secondary :disabled="taskStore.isCreating" @click="selectSaveDir">选择目录</n-button>
          </n-space>
        </n-form-item>

        <n-form-item label="开始方式">
          <n-tabs v-model:value="form.startMode" type="segment">
            <n-tab-pane name="now" tab="立即开始" />
            <n-tab-pane name="paused" tab="添加后暂停" />
          </n-tabs>
        </n-form-item>

        <n-form-item label="备注">
          <n-input v-model:value="form.note" placeholder="可选" />
        </n-form-item>

        <n-collapse>
          <n-collapse-item title="高级设置" name="advanced">
            <n-grid :cols="2" :x-gap="12" :y-gap="12">
              <n-gi><n-input placeholder="分类：默认" disabled /></n-gi>
              <n-gi><n-input placeholder="连接数：16" disabled /></n-gi>
              <n-gi><n-input placeholder="限速：不限速" disabled /></n-gi>
              <n-gi><n-input placeholder="代理：后期支持" disabled /></n-gi>
            </n-grid>
          </n-collapse-item>
        </n-collapse>

        <n-alert v-if="formErrorMessage" type="error" class="form-alert">{{ formErrorMessage }}</n-alert>

        <n-space justify="end" class="dialog-actions">
          <n-button :disabled="taskStore.isCreating" @click="closeDialog">取消</n-button>
          <n-button type="primary" attr-type="submit" :loading="taskStore.isCreating" :disabled="!isUrlValid">开始下载</n-button>
        </n-space>
      </n-form>
    </n-card>
  </n-modal>
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

.form-alert {
  margin-top: 16px;
}

.dialog-actions {
  margin-top: 22px;
}
</style>
