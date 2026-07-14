<script setup lang="ts">
import {
  IconActivityHeartbeat,
  IconAlertTriangle,
  IconCheck,
  IconCircleCheck,
  IconDownload,
  IconFileInfo,
  IconHelpCircle,
  IconInfoCircle,
  IconList,
  IconLogout,
  IconPlayerPause,
  IconPlayerPlay,
  IconPlus,
  IconPuzzle,
  IconRefresh,
  IconReload,
  IconSettings,
  IconTrash,
  IconTrashX,
} from "@tabler/icons-vue";
import { computed, type Component } from "vue";

const props = withDefaults(
  defineProps<{
    name: string;
    size?: number | string;
    decorative?: boolean;
  }>(),
  {
    size: 18,
    decorative: true,
  },
);

const icons: Record<string, Component> = {
  all: IconList,
  download: IconDownload,
  completed: IconCircleCheck,
  trash: IconTrash,
  extensions: IconPuzzle,
  plus: IconPlus,
  refresh: IconRefresh,
  play: IconPlayerPlay,
  pause: IconPlayerPause,
  info: IconFileInfo,
  confirm: IconCheck,
  redownload: IconReload,
  delete: IconTrash,
  permanentDelete: IconTrashX,
  settings: IconSettings,
  help: IconHelpCircle,
  about: IconInfoCircle,
  diagnostics: IconActivityHeartbeat,
  logout: IconLogout,
};

const iconComponent = computed(() => icons[props.name] ?? IconAlertTriangle);
const normalizedSize = computed(() => (typeof props.size === "number" ? `${props.size}px` : props.size));
const ariaHidden = computed(() => (props.decorative ? "true" : undefined));
const role = computed(() => (props.decorative ? undefined : "img"));
</script>

<template>
  <component
    :is="iconComponent"
    class="app-icon"
    :data-icon-name="icons[props.name] ? props.name : 'unknown'"
    :size="normalizedSize"
    :aria-hidden="ariaHidden"
    :role="role"
    :aria-label="props.decorative ? undefined : props.name"
    :focusable="false"
  />
</template>

<style scoped>
.app-icon {
  display: inline-block;
  flex: 0 0 auto;
  color: currentColor;
  vertical-align: -0.125em;
}
</style>
