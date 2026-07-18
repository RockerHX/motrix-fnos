<script setup lang="ts">
import { computed } from "vue";
import AppIcon from "../../../components/AppIcon.vue";
import { t } from "../../../i18n";
import type { DownloadTaskSourceType } from "../../../types/tasks";

const props = defineProps<{
  sourceType?: DownloadTaskSourceType;
  url: string;
}>();

const sourceType = computed<DownloadTaskSourceType>(() => {
  if (props.sourceType) {
    return props.sourceType;
  }

  const url = props.url.trim().toLowerCase();
  if (url.startsWith("magnet:?")) {
    return "magnet";
  }
  if (url.startsWith("torrent:")) {
    return "torrent";
  }
  return "url";
});

const sourceLabel = computed(() => t(`task.source.${sourceType.value}` as const));
</script>

<template>
  <span class="task-source-icon" role="img" :aria-label="sourceLabel" :title="sourceLabel">
    <AppIcon :name="sourceType === 'url' ? 'link' : sourceType" :size="16" />
  </span>
</template>
