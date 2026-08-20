<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { NAlert, NButton, useMessage } from "naive-ui";
import { copyTextToClipboard } from "../../../app/utils/clipboard";
import { useI18n } from "../../../i18n";
import { useJsonRpcTokenStore } from "../stores/jsonRpcTokenStore";
import { useLanJsonRpcStore } from "../stores/lanJsonRpcStore";
import { lanJsonRpcEndpoint } from "../utils/lanJsonRpcEndpoint";

const PROXY_RPC_ENDPOINT = "http://127.0.0.1:17081/jsonrpc";

const emit = defineEmits<{
  openSettings: [];
}>();

const { t } = useI18n();
const message = useMessage();
const publicTokenStore = useJsonRpcTokenStore();
const lanStore = useLanJsonRpcStore();
const lanEndpoint = computed(() => lanJsonRpcEndpoint(window.location.hostname));
const copyTarget = ref<"proxy" | "lan" | null>(null);
const manualCopyTarget = ref<"proxy" | "lan" | null>(null);
let copyResetTimer: number | undefined;

async function copyEndpoint(target: "proxy" | "lan", value: string) {
  const result = await copyTextToClipboard(value);
  if (result.copied) {
    copyTarget.value = target;
    manualCopyTarget.value = null;
    if (copyResetTimer !== undefined) {
      window.clearTimeout(copyResetTimer);
    }
    copyResetTimer = window.setTimeout(() => {
      copyTarget.value = null;
      copyResetTimer = undefined;
    }, 2200);
    return;
  }

  copyTarget.value = null;
  manualCopyTarget.value = target;
  message.warning(t("common.clipboardManualCopy"));
}

function tokenStatus(configured: boolean | undefined) {
  if (configured === undefined) return t("rpcGuide.unknown");
  return t(configured ? "rpcGuide.configured" : "rpcGuide.notConfigured");
}

onMounted(() => {
  void Promise.allSettled([publicTokenStore.loadStatus(), lanStore.loadStatus()]);
});

onUnmounted(() => {
  if (copyResetTimer !== undefined) {
    window.clearTimeout(copyResetTimer);
  }
});
</script>

<template>
  <section class="json-rpc-guide">
    <header class="json-rpc-guide-header">
      <div>
        <h3>{{ t("rpcGuide.title") }}</h3>
        <p>{{ t("rpcGuide.description") }}</p>
      </div>
      <NButton secondary size="small" @click="emit('openSettings')">
        {{ t("rpcGuide.openSettings") }}
      </NButton>
    </header>

    <NAlert type="warning" :bordered="false" class="json-rpc-guide-alert">
      {{ t("rpcGuide.portWarning") }}
    </NAlert>

    <NAlert type="info" :bordered="false">
      {{ t("rpcGuide.protocolScope") }}
    </NAlert>

    <div class="json-rpc-guide-steps">
      <section>
        <span class="json-rpc-guide-step">1</span>
        <div>
          <h4>{{ t("rpcGuide.stepToken.title") }}</h4>
          <p>{{ t("rpcGuide.stepToken.body") }}</p>
        </div>
      </section>
      <section>
        <span class="json-rpc-guide-step">2</span>
        <div>
          <h4>{{ t("rpcGuide.stepLocal.title") }}</h4>
          <p>{{ t("rpcGuide.stepLocal.body") }}</p>
        </div>
      </section>
      <section>
        <span class="json-rpc-guide-step">3</span>
        <div>
          <h4>{{ t("rpcGuide.stepRemote.title") }}</h4>
          <p>{{ t("rpcGuide.stepRemote.body") }}</p>
        </div>
      </section>
    </div>

    <div class="json-rpc-guide-endpoint">
      <div>
        <span class="json-rpc-guide-label">{{ t("rpcGuide.proxyEndpoint") }}</span>
        <code data-test="json-rpc-proxy-endpoint">{{ PROXY_RPC_ENDPOINT }}</code>
        <small>{{ t("rpcGuide.publicTokenStatus", { status: tokenStatus(publicTokenStore.status?.configured) }) }}</small>
      </div>
      <NButton size="small" secondary @click="copyEndpoint('proxy', PROXY_RPC_ENDPOINT)">
        {{
          copyTarget === "proxy"
            ? t("rpcGuide.copied")
            : manualCopyTarget === "proxy"
              ? t("rpcGuide.copyUnavailable")
              : t("rpcGuide.copyEndpoint")
        }}
      </NButton>
    </div>

    <div class="json-rpc-guide-endpoint">
      <div>
        <span class="json-rpc-guide-label">{{ t("rpcGuide.lanEndpoint") }}</span>
        <code data-test="json-rpc-lan-endpoint">{{ lanEndpoint.value }}</code>
        <small v-if="!lanEndpoint.concrete">{{ t("rpcGuide.lanAddressHint") }}</small>
        <small>
          {{
            t("rpcGuide.lanTokenStatus", {
              enabled: t(lanStore.status?.enabled ? "rpcGuide.enabled" : "rpcGuide.disabled"),
              status: tokenStatus(lanStore.status?.configured),
            })
          }}
        </small>
      </div>
      <NButton
        v-if="lanEndpoint.concrete"
        size="small"
        secondary
        @click="copyEndpoint('lan', lanEndpoint.value)"
      >
        {{
          copyTarget === "lan"
            ? t("rpcGuide.copied")
            : manualCopyTarget === "lan"
              ? t("rpcGuide.copyUnavailable")
              : t("rpcGuide.copyEndpoint")
        }}
      </NButton>
    </div>

    <p class="json-rpc-guide-note">{{ t("rpcGuide.securityNote") }}</p>
  </section>
</template>

<style scoped src="./JsonRpcGuide.css"></style>
