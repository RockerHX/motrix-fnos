<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NAlert, NButton, NInput, NModal, NSpace, NText, useMessage } from "naive-ui";
import { getErrorMessage } from "../../../app/utils/errors";
import { useI18n } from "../../../i18n";
import { useJsonRpcTokenStore } from "../stores/jsonRpcTokenStore";

const props = defineProps<{ active: boolean }>();
const emit = defineEmits<{
  openGuide: [];
}>();
const tokenStore = useJsonRpcTokenStore();
const message = useMessage();
const { t } = useI18n();
const tokenVisible = ref(false);
const showClearConfirm = ref(false);
const statusText = computed(() =>
  tokenStore.status?.configured
    ? tokenStore.status.maskedToken || t("settings.jsonRpcToken.configured")
    : t("settings.jsonRpcToken.notConfigured"),
);

watch(
  () => props.active,
  (active) => {
    if (active) {
      void loadStatus();
    } else {
      tokenStore.clearDraft();
      tokenVisible.value = false;
      showClearConfirm.value = false;
    }
  },
  { immediate: true },
);

async function loadStatus() {
  try {
    await tokenStore.loadStatus();
  } catch (error) {
    message.error(getErrorMessage(error, t("settings.jsonRpcToken.loadFailed")));
  }
}

async function saveToken() {
  try {
    await tokenStore.saveDraft();
    tokenVisible.value = false;
    message.success(t("settings.jsonRpcToken.saved"));
  } catch (error) {
    message.error(getErrorMessage(error, t("settings.jsonRpcToken.saveFailed")));
  }
}

async function clearToken() {
  try {
    await tokenStore.clearToken();
    showClearConfirm.value = false;
    tokenVisible.value = false;
    message.success(t("settings.jsonRpcToken.cleared"));
  } catch (error) {
    message.error(getErrorMessage(error, t("settings.jsonRpcToken.saveFailed")));
  }
}
</script>

<template>
  <section class="json-rpc-settings" data-test="json-rpc-token-settings">
    <div class="json-rpc-heading">
      <div>
        <h3>{{ t("settings.jsonRpcToken") }}</h3>
        <NText depth="3">{{ t("settings.jsonRpcToken.help") }}</NText>
      </div>
      <NText :type="tokenStore.status?.configured ? 'success' : 'warning'" data-test="json-rpc-token-status">
        {{ tokenStore.isLoading ? t("common.loading") : statusText }}
      </NText>
    </div>

    <NAlert type="info" :bordered="false">{{ t("settings.jsonRpcToken.maskHelp") }}</NAlert>

    <NAlert type="warning" :bordered="false" class="json-rpc-port-hint">
      <span>{{ t("settings.jsonRpcToken.portHelp") }}</span>
      <NButton text type="primary" size="small" @click="emit('openGuide')">
        {{ t("settings.jsonRpcToken.openGuide") }}
      </NButton>
    </NAlert>

    <NInput
      v-model:value="tokenStore.draftToken"
      :type="tokenVisible ? 'text' : 'password'"
      :placeholder="t('settings.jsonRpcToken.placeholder')"
      :disabled="tokenStore.isSaving"
      :input-props="{ autocomplete: 'off', spellcheck: 'false' }"
      data-test="json-rpc-token-input"
    />

    <NSpace justify="space-between" wrap>
      <NSpace wrap>
        <NButton :disabled="tokenStore.isSaving" @click="tokenStore.generateToken">
          {{ t(tokenStore.status?.configured ? "settings.jsonRpcToken.rotate" : "settings.jsonRpcToken.generate") }}
        </NButton>
        <NButton :disabled="!tokenStore.draftToken || tokenStore.isSaving" @click="tokenVisible = !tokenVisible">
          {{ t(tokenVisible ? "settings.jsonRpcToken.hide" : "settings.jsonRpcToken.show") }}
        </NButton>
      </NSpace>
      <NSpace wrap>
        <NButton
          type="error"
          secondary
          :disabled="!tokenStore.status?.configured || tokenStore.isSaving"
          @click="showClearConfirm = true"
        >
          {{ t("settings.jsonRpcToken.clear") }}
        </NButton>
        <NButton type="primary" :loading="tokenStore.isSaving" :disabled="!tokenStore.draftToken" @click="saveToken">
          {{ t("settings.jsonRpcToken.save") }}
        </NButton>
      </NSpace>
    </NSpace>

    <NModal
      v-model:show="showClearConfirm"
      preset="card"
      class="json-rpc-clear-modal"
      :title="t('settings.jsonRpcToken.clearTitle')"
      :mask-closable="!tokenStore.isSaving"
      :closable="!tokenStore.isSaving"
    >
      <NAlert type="warning" :title="t('settings.jsonRpcToken.clearTitle')">
        {{ t("settings.jsonRpcToken.clearConfirm") }}
      </NAlert>
      <NSpace justify="end" class="json-rpc-clear-actions">
        <NButton :disabled="tokenStore.isSaving" @click="showClearConfirm = false">{{ t("common.cancel") }}</NButton>
        <NButton type="error" :loading="tokenStore.isSaving" @click="clearToken">
          {{ t("settings.jsonRpcToken.clear") }}
        </NButton>
      </NSpace>
    </NModal>
  </section>
</template>

<style scoped src="./JsonRpcTokenSettings.css"></style>
