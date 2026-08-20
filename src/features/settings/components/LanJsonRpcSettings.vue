<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from "vue";
import { IconCopy, IconRefresh } from "@tabler/icons-vue";
import {
  NAlert,
  NButton,
  NIcon,
  NInput,
  NSpace,
  NSwitch,
  NText,
  useMessage,
  type InputInst,
} from "naive-ui";
import AppDialog from "../../../components/ui/AppDialog.vue";
import AppDialogActions from "../../../components/ui/AppDialogActions.vue";
import { copyTextToClipboard } from "../../../app/utils/clipboard";
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
const issuedTokenInput = ref<InputInst | null>(null);
const endpoint = computed(() => lanJsonRpcEndpoint(window.location.hostname));

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

watch(
  () => store.issuedToken,
  (token) => {
    if (token) {
      void selectIssuedToken();
    }
  },
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

async function copyText(value: string, selectTokenOnFailure = false) {
  const result = await copyTextToClipboard(value);
  if (result.copied) {
    message.success(t("common.copied"));
    return;
  }

  message.warning(t("common.clipboardManualCopy"));
  if (selectTokenOnFailure) {
    await selectIssuedToken();
  }
}

async function selectIssuedToken() {
  await nextTick();
  issuedTokenInput.value?.focus();
  issuedTokenInput.value?.select();
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
        <NSwitch
          :value="store.status?.enabled ?? false"
          :loading="store.isSaving"
          :disabled="store.isLoading || store.isSaving"
          :aria-label="t('settings.lanJsonRpc.toggle')"
          :title="t('settings.lanJsonRpc.toggle')"
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

    <section v-if="store.status?.enabled" class="lan-json-rpc-token-card" data-test="lan-json-rpc-token-card">
      <div>
        <span>{{ t("settings.lanJsonRpc.token") }}</span>
        <code data-test="lan-json-rpc-masked-token">
          {{ store.status.maskedToken ?? t("settings.lanJsonRpc.notConfigured") }}
        </code>
        <small data-test="lan-json-rpc-token-status">
          {{ t(store.status.configured ? "settings.lanJsonRpc.configured" : "settings.lanJsonRpc.notConfigured") }}
        </small>
        <small>{{ t("settings.lanJsonRpc.tokenHint") }}</small>
      </div>
      <NButton
        secondary
        :loading="store.isSaving"
        :disabled="store.isLoading || store.isSaving"
        data-test="rotate-lan-json-rpc-token"
        @click="showRotateConfirm = true"
      >
        <template #icon><NIcon><IconRefresh /></NIcon></template>
        {{ t("settings.lanJsonRpc.rotate") }}
      </NButton>
    </section>

    <NSpace justify="space-between" wrap>
      <NButton text type="primary" @click="emit('openGuide')">
        {{ t("settings.jsonRpcToken.openGuide") }}
      </NButton>
    </NSpace>

    <AppDialog
      :show="Boolean(store.issuedToken)"
      :title="t('settings.lanJsonRpc.issuedTitle')"
      width="520px"
      @update:show="!$event && closeIssuedToken()"
    >
      <NAlert type="warning" :bordered="false">{{ t("settings.lanJsonRpc.issuedWarning") }}</NAlert>
      <NInput
        ref="issuedTokenInput"
        class="lan-json-rpc-issued-input"
        :value="store.issuedToken"
        readonly
        type="text"
        :input-props="{ autocomplete: 'off', spellcheck: 'false' }"
        data-test="lan-json-rpc-issued-token"
      />
      <template #footer>
        <AppDialogActions>
          <NButton secondary @click="copyText(store.issuedToken, true)">
            <template #icon><NIcon><IconCopy /></NIcon></template>
            {{ t("common.copy") }}
          </NButton>
          <NButton type="primary" @click="closeIssuedToken">{{ t("common.done") }}</NButton>
        </AppDialogActions>
      </template>
    </AppDialog>

    <AppDialog
      :show="showRotateConfirm"
      :title="t('settings.lanJsonRpc.rotateTitle')"
      width="520px"
      :mask-closable="!store.isSaving"
      :close-disabled="store.isSaving"
      @update:show="showRotateConfirm = $event"
    >
      <NAlert type="warning" :bordered="false">{{ t("settings.lanJsonRpc.rotateConfirm") }}</NAlert>
      <template #footer>
        <AppDialogActions>
          <NButton :disabled="store.isSaving" @click="showRotateConfirm = false">{{ t("common.cancel") }}</NButton>
          <NButton type="warning" :loading="store.isSaving" @click="rotateToken">
            {{ t("settings.lanJsonRpc.rotate") }}
          </NButton>
        </AppDialogActions>
      </template>
    </AppDialog>
  </section>
</template>

<style scoped src="./LanJsonRpcSettings.css"></style>
