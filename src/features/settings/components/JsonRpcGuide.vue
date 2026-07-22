<script setup lang="ts">
import { onUnmounted, ref } from "vue";
import { NAlert, NButton } from "naive-ui";
import { useI18n } from "../../../i18n";

const LOCAL_RPC_ENDPOINT = "http://127.0.0.1:17081/jsonrpc";

const emit = defineEmits<{
  openSettings: [];
}>();

const { t } = useI18n();
const copyState = ref<"idle" | "copied" | "unavailable">("idle");
let copyResetTimer: number | undefined;

async function copyEndpoint() {
  if (!navigator.clipboard?.writeText) {
    copyState.value = "unavailable";
    return;
  }

  try {
    await navigator.clipboard.writeText(LOCAL_RPC_ENDPOINT);
    copyState.value = "copied";
    if (copyResetTimer !== undefined) {
      window.clearTimeout(copyResetTimer);
    }
    copyResetTimer = window.setTimeout(() => {
      copyState.value = "idle";
      copyResetTimer = undefined;
    }, 2200);
  } catch {
    copyState.value = "unavailable";
  }
}

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
        <span class="json-rpc-guide-label">{{ t("rpcGuide.localEndpoint") }}</span>
        <code data-test="json-rpc-local-endpoint">{{ LOCAL_RPC_ENDPOINT }}</code>
      </div>
      <NButton size="small" secondary @click="copyEndpoint">
        {{
          copyState === "copied"
            ? t("rpcGuide.copied")
            : copyState === "unavailable"
              ? t("rpcGuide.copyUnavailable")
              : t("rpcGuide.copyEndpoint")
        }}
      </NButton>
    </div>

    <p class="json-rpc-guide-note">{{ t("rpcGuide.securityNote") }}</p>
  </section>
</template>

<style scoped src="./JsonRpcGuide.css"></style>
