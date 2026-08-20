<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { IconDeviceFloppy, IconTrash } from "@tabler/icons-vue";
import { NAlert, NButton, NIcon, NInput, NSpace, NText, useMessage } from "naive-ui";
import AppDialog from "../../../components/ui/AppDialog.vue";
import AppDialogActions from "../../../components/ui/AppDialogActions.vue";
import { getErrorMessage } from "../../../app/utils/errors";
import { useI18n } from "../../../i18n";
import { useDownloadProxyStore } from "../stores/downloadProxyStore";

const props = defineProps<{ active: boolean }>();
const store = useDownloadProxyStore();
const message = useMessage();
const { t } = useI18n();
const showSaveConfirm = ref(false);
const showClearConfirm = ref(false);

const statusText = computed(() => {
  if (store.isLoading) return t("common.loading");
  if (!store.status) return t("settings.proxy.statusUnknown");
  return store.status.configured
    ? store.status.maskedProxyUrl || t("settings.proxy.configured")
    : t("settings.proxy.notConfigured");
});
const canSave = computed(() => Boolean(store.draftProxyUrl.trim()) && !store.isLoading && !store.isSaving);
const saveMode = computed(() => (store.status?.configured ? "replace" : "save"));
const mutationSummary = computed(() => {
  const result = store.lastMutation;
  if (!result) return "";
  return t("settings.proxy.resultSummary", {
    applied: result.appliedTaskIds.length,
    deferred: result.deferredTaskIds.length,
    failed: result.failed.length,
  });
});

watch(
  () => props.active,
  (active) => {
    if (active) {
      void loadStatus();
    } else {
      closeTransientState();
    }
  },
  { immediate: true },
);

async function loadStatus() {
  try {
    await store.loadStatus();
  } catch (error) {
    message.error(getErrorMessage(error, t("settings.proxy.loadFailed")));
  }
}

function requestSave() {
  if (!canSave.value) return;
  showSaveConfirm.value = true;
}

async function saveProxy() {
  const mode = saveMode.value;
  try {
    const result = await store.saveDraft();
    if (!result) return;
    message.success(t(mode === "replace" ? "settings.proxy.replaced" : "settings.proxy.saved"));
  } catch (error) {
    message.error(getErrorMessage(error, t("settings.proxy.saveFailed")));
  } finally {
    showSaveConfirm.value = false;
  }
}

async function clearProxy() {
  try {
    await store.clearProxy();
    showClearConfirm.value = false;
    message.success(t("settings.proxy.cleared"));
  } catch (error) {
    message.error(getErrorMessage(error, t("settings.proxy.clearFailed")));
  }
}

function closeTransientState() {
  showSaveConfirm.value = false;
  showClearConfirm.value = false;
  store.clearTransientState();
}

onUnmounted(closeTransientState);
</script>

<template>
  <section class="proxy-settings" data-test="download-proxy-settings">
    <div class="proxy-settings-heading">
      <div>
        <h3>{{ t("settings.proxy.title") }}</h3>
        <NText depth="3">{{ t("settings.proxy.help") }}</NText>
      </div>
      <NText :type="store.status?.configured ? 'success' : 'warning'" data-test="download-proxy-status">
        {{ statusText }}
      </NText>
    </div>

    <NAlert type="info" :bordered="false">{{ t("settings.proxy.privacyHelp") }}</NAlert>

    <NInput
      :value="store.draftProxyUrl"
      type="password"
      show-password-on="mousedown"
      :placeholder="t('settings.proxy.placeholder')"
      :disabled="store.isLoading || store.isSaving"
      :input-props="{ autocomplete: 'off', spellcheck: 'false' }"
      :maxlength="2048"
      data-test="download-proxy-input"
      @update:value="store.setDraftProxyUrl"
      @keyup.enter="requestSave"
    />

    <NSpace justify="space-between" align="center" wrap class="proxy-settings-actions">
      <NButton
        type="error"
        secondary
        :disabled="!store.status?.configured || store.isLoading || store.isSaving"
        @click="showClearConfirm = true"
      >
        <template #icon><NIcon><IconTrash /></NIcon></template>
        {{ t("settings.proxy.clear") }}
      </NButton>
      <NButton type="primary" :loading="store.isSaving" :disabled="!canSave" @click="requestSave">
        <template #icon><NIcon><IconDeviceFloppy /></NIcon></template>
        {{ t(store.status?.configured ? "settings.proxy.replace" : "settings.proxy.save") }}
      </NButton>
    </NSpace>

    <NAlert
      v-if="store.lastMutation"
      :type="store.lastMutation.failed.length > 0 ? 'warning' : 'success'"
      :title="t('settings.proxy.resultTitle')"
      :bordered="false"
      data-test="download-proxy-result"
    >
      <p>{{ mutationSummary }}</p>
      <ul v-if="store.lastMutation.failed.length > 0" class="proxy-settings-failures">
        <li v-for="failure in store.lastMutation.failed" :key="failure.taskId">
          #{{ failure.taskId }} · {{ failure.message }}
        </li>
      </ul>
    </NAlert>

    <AppDialog
      :show="showSaveConfirm"
      :title="t(saveMode === 'replace' ? 'settings.proxy.replaceTitle' : 'settings.proxy.saveTitle')"
      width="500px"
      :mask-closable="!store.isSaving"
      :close-disabled="store.isSaving"
      @update:show="showSaveConfirm = $event"
    >
      <NAlert type="warning" :bordered="false">
        {{ t(saveMode === "replace" ? "settings.proxy.replaceConfirm" : "settings.proxy.saveConfirm") }}
      </NAlert>
      <template #footer>
        <AppDialogActions>
          <NButton :disabled="store.isSaving" @click="showSaveConfirm = false">{{ t("common.cancel") }}</NButton>
          <NButton type="primary" :loading="store.isSaving" @click="saveProxy">
            <template #icon><NIcon><IconDeviceFloppy /></NIcon></template>
            {{ t(saveMode === "replace" ? "settings.proxy.replace" : "settings.proxy.save") }}
          </NButton>
        </AppDialogActions>
      </template>
    </AppDialog>

    <AppDialog
      :show="showClearConfirm"
      :title="t('settings.proxy.clearTitle')"
      width="500px"
      :mask-closable="!store.isSaving"
      :close-disabled="store.isSaving"
      @update:show="showClearConfirm = $event"
    >
      <NAlert type="warning" :bordered="false">{{ t("settings.proxy.clearConfirm") }}</NAlert>
      <template #footer>
        <AppDialogActions>
          <NButton :disabled="store.isSaving" @click="showClearConfirm = false">{{ t("common.cancel") }}</NButton>
          <NButton type="error" :loading="store.isSaving" @click="clearProxy">
            <template #icon><NIcon><IconTrash /></NIcon></template>
            {{ t("settings.proxy.clear") }}
          </NButton>
        </AppDialogActions>
      </template>
    </AppDialog>
  </section>
</template>

<style scoped src="./ProxySettings.css"></style>
