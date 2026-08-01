<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { IconCopy, IconRefresh } from "@tabler/icons-vue";
import { NAlert, NButton, NIcon, NInput, NModal, NSpace, NSwitch, NText, useMessage } from "naive-ui";
import { getErrorMessage } from "../../../app/utils/errors";
import { useI18n } from "../../../i18n";
import { useLanJsonRpcStore } from "../stores/lanJsonRpcStore";
import { lanJsonRpcEndpoint } from "../utils/lanJsonRpcEndpoint";

const props = defineProps<{ active: boolean }>();
const emit = defineEmits<{ openGuide: [] }>();
const store = useLanJsonRpcStore();
const message = useMessage();
const { t } = useI18n();
const showRotateConfirm = ref(false);
const endpoint = computed(() => lanJsonRpcEndpoint(window.location.hostname));
const statusText = computed(() => {
  if (store.isLoading) return t("common.loading");
  if (!store.status?.enabled) return t("settings.lanJsonRpc.disabled");
  return store.status.configured
    ? store.status.maskedToken || t("settings.lanJsonRpc.configured")
    : t("settings.lanJsonRpc.notConfigured");
});

watch(
  () => props.active,
  (active) => {
    if (active) {
      void loadStatus();
    } else {
      closeSensitiveDialogs();
    }
  },
  { immediate: true },
);

async function loadStatus() {
  try {
    await store.loadStatus();
  } catch (error) {
    message.error(getErrorMessage(error, t("settings.lanJsonRpc.loadFailed")));
  }
}

async function updateEnabled(enabled: boolean) {
  try {
    await store.setEnabled(enabled);
    message.success(t(enabled ? "settings.lanJsonRpc.enabled" : "settings.lanJsonRpc.disabledSuccess"));
  } catch (error) {
    message.error(getErrorMessage(error, t("settings.lanJsonRpc.saveFailed")));
  }
}

async function rotateToken() {
  try {
    await store.rotateToken();
    showRotateConfirm.value = false;
    message.success(t("settings.lanJsonRpc.rotated"));
  } catch (error) {
    message.error(getErrorMessage(error, t("settings.lanJsonRpc.rotateFailed")));
  }
}

async function copyText(value: string) {
  try {
    await navigator.clipboard.writeText(value);
    message.success(t("common.copied"));
  } catch (error) {
    message.error(getErrorMessage(error, t("settings.lanJsonRpc.copyFailed")));
  }
}

function closeIssuedToken() {
  store.clearIssuedToken();
}

function closeSensitiveDialogs() {
  showRotateConfirm.value = false;
  store.clearSensitiveState();
}

onUnmounted(closeSensitiveDialogs);
</script>

<template>
  <section class="lan-json-rpc-settings" data-test="lan-json-rpc-settings">
    <div class="lan-json-rpc-heading">
      <div>
        <h3>{{ t("settings.lanJsonRpc.title") }}</h3>
        <NText depth="3">{{ t("settings.lanJsonRpc.help") }}</NText>
      </div>
      <NSpace align="center" :wrap="false">
        <NText :type="store.status?.enabled ? 'success' : 'default'" data-test="lan-json-rpc-status">
          {{ statusText }}
        </NText>
        <NSwitch
          :value="store.status?.enabled ?? false"
          :loading="store.isSaving"
          :disabled="store.isLoading || store.isSaving"
          data-test="lan-json-rpc-switch"
          @update:value="updateEnabled"
        />
      </NSpace>
    </div>

    <NAlert type="info" :bordered="false">
      {{ t("settings.lanJsonRpc.security") }}
    </NAlert>

    <div class="lan-json-rpc-endpoint">
      <div>
        <span>{{ t("settings.lanJsonRpc.endpoint") }}</span>
        <code data-test="lan-json-rpc-endpoint">{{ endpoint.value }}</code>
        <small v-if="!endpoint.concrete">{{ t("settings.lanJsonRpc.endpointHint") }}</small>
      </div>
      <NButton
        v-if="endpoint.concrete"
        circle
        secondary
        :aria-label="t('settings.lanJsonRpc.copyEndpoint')"
        :title="t('settings.lanJsonRpc.copyEndpoint')"
        data-test="copy-lan-json-rpc-endpoint"
        @click="copyText(endpoint.value)"
      >
        <template #icon><NIcon><IconCopy /></NIcon></template>
      </NButton>
    </div>

    <NSpace justify="space-between" wrap>
      <NButton text type="primary" @click="emit('openGuide')">
        {{ t("settings.jsonRpcToken.openGuide") }}
      </NButton>
      <NButton
        secondary
        :loading="store.isSaving"
        :disabled="store.isLoading || store.isSaving"
        @click="showRotateConfirm = true"
      >
        <template #icon><NIcon><IconRefresh /></NIcon></template>
        {{ t("settings.lanJsonRpc.rotate") }}
      </NButton>
    </NSpace>

    <NModal
      :show="Boolean(store.issuedToken)"
      preset="card"
      class="lan-json-rpc-modal"
      :title="t('settings.lanJsonRpc.issuedTitle')"
      @update:show="!$event && closeIssuedToken()"
    >
      <NAlert type="warning" :bordered="false">{{ t("settings.lanJsonRpc.issuedWarning") }}</NAlert>
      <NInput
        :value="store.issuedToken"
        readonly
        type="text"
        :input-props="{ autocomplete: 'off', spellcheck: 'false' }"
        data-test="lan-json-rpc-issued-token"
      />
      <NSpace justify="end" class="lan-json-rpc-actions">
        <NButton secondary @click="copyText(store.issuedToken)">
          <template #icon><NIcon><IconCopy /></NIcon></template>
          {{ t("common.copy") }}
        </NButton>
        <NButton type="primary" @click="closeIssuedToken">{{ t("common.done") }}</NButton>
      </NSpace>
    </NModal>

    <NModal
      v-model:show="showRotateConfirm"
      preset="card"
      class="lan-json-rpc-modal"
      :title="t('settings.lanJsonRpc.rotateTitle')"
      :mask-closable="!store.isSaving"
      :closable="!store.isSaving"
    >
      <NAlert type="warning" :bordered="false">{{ t("settings.lanJsonRpc.rotateConfirm") }}</NAlert>
      <NSpace justify="end" class="lan-json-rpc-actions">
        <NButton :disabled="store.isSaving" @click="showRotateConfirm = false">{{ t("common.cancel") }}</NButton>
        <NButton type="warning" :loading="store.isSaving" @click="rotateToken">
          {{ t("settings.lanJsonRpc.rotate") }}
        </NButton>
      </NSpace>
    </NModal>
  </section>
</template>

<style scoped src="./LanJsonRpcSettings.css"></style>
