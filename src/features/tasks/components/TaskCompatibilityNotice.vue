<script setup lang="ts">
import { computed } from "vue";
import { NAlert } from "naive-ui";
import { useTaskStore } from "../stores/taskStore";
import { useI18n } from "../../../i18n";
import { formatServerMessage } from "../../../i18n/serverMessages";

const store = useTaskStore();
const { t } = useI18n();
const hasReadOnlyHistory = computed(() => [...store.tasks, ...store.removedTasks].some((task) => task.readOnly));
</script>

<template>
  <NAlert v-if="store.isDownloadBlocked" type="warning" :title="t('task.upgradeRequired')" :closable="false">
    {{ formatServerMessage(store.compatibility?.message ?? "", "engine_upgrade_required") }}
  </NAlert>
  <NAlert v-else-if="hasReadOnlyHistory" type="info" :closable="false">
    {{ t("task.legacyReadOnly") }}
  </NAlert>
</template>
