<script setup lang="ts">
import { computed } from "vue";

interface IconDefinition {
  viewBox: string;
  body: string;
}

function icon(body: string, viewBox = "0 0 24 24"): IconDefinition {
  return { viewBox, body };
}

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

const icons: Record<string, IconDefinition> = {
  download: icon(`<path d="M12 3v11"/><path d="m7 10 5 5 5-5"/><path d="M5 19h14"/>`),
  completed: icon(`<path d="m5 12 4 4L19 6"/>`),
  stopped: icon(`<path d="M8 5v14"/><path d="M16 5v14"/>`),
  trash: icon(`<path d="M4 7h16"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M6 7l1 14h10l1-14"/><path d="M9 7V4h6v3"/>`),
  extensions: icon(`<path d="M9 4a3 3 0 0 1 6 0v2h3a2 2 0 0 1 2 2v4h-3a3 3 0 1 0 0 6h3v2a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-3h2a3 3 0 1 0 0-6H4V8a2 2 0 0 1 2-2h3V4Z"/>`),
  plus: icon(`<path d="M12 5v14"/><path d="M5 12h14"/>`),
  refresh: icon(`<path d="M20 12a8 8 0 1 1-2.35-5.65"/><path d="M20 4v6h-6"/>`),
  play: icon(`<path d="m8 5 11 7-11 7V5Z" fill="currentColor" stroke="none"/>`),
  pause: icon(`<path d="M9 6v12"/><path d="M15 6v12"/>`),
  close: icon(`<path d="m6 6 12 12"/><path d="m18 6-12 12"/>`),
  more: icon(`<circle cx="6" cy="12" r="1.4" fill="currentColor" stroke="none"/><circle cx="12" cy="12" r="1.4" fill="currentColor" stroke="none"/><circle cx="18" cy="12" r="1.4" fill="currentColor" stroke="none"/>`),
  info: icon(`<path d="M12 11v6"/><path d="M12 7h.01"/>`),
  confirm: icon(`<path d="m5 12 4 4L19 6"/>`),
  redownload: icon(`<path d="M20 12a8 8 0 1 1-2.35-5.65"/><path d="M20 4v6h-6"/>`),
  delete: icon(`<path d="m6 6 12 12"/><path d="m18 6-12 12"/>`),
  permanentDelete: icon(`<path d="M4 7h16"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M6 7l1 14h10l1-14"/><path d="M9 7V4h6v3"/>`),
  settings: icon(`<path d="M12 15.5A3.5 3.5 0 1 0 12 8a3.5 3.5 0 0 0 0 7.5Z"/><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06A1.7 1.7 0 0 0 15 19.36a1.7 1.7 0 0 0-1 .58V20a2 2 0 1 1-4 0v-.08A1.7 1.7 0 0 0 9 19.34a1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.64 15a1.7 1.7 0 0 0-.58-1H4a2 2 0 1 1 0-4h.08A1.7 1.7 0 0 0 4.66 9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.64a1.7 1.7 0 0 0 1-.58V4a2 2 0 1 1 4 0v.08a1.7 1.7 0 0 0 1 .58 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.36 9c.08.36.28.7.58 1H20a2 2 0 1 1 0 4h-.08a1.7 1.7 0 0 0-.52 1Z"/>`),
  help: icon(`<path d="M9.1 9a3 3 0 1 1 5.8 1c-.5 1-1.6 1.5-2.3 2.2-.4.4-.6.9-.6 1.8"/><path d="M12 18h.01"/>`),
  about: icon(`<circle cx="12" cy="12" r="9"/><path d="M12 11v5"/><path d="M12 8h.01"/>`),
  diagnostics: icon(`<path d="M4 17h16"/><path d="M7 17V9"/><path d="M12 17V5"/><path d="M17 17v-6"/>`),
};
const fallbackIcon = icon(`<circle cx="12" cy="12" r="8"/><path d="M9 9l6 6"/><path d="m15 9-6 6"/>`);

const iconDefinition = computed(() => icons[props.name] ?? fallbackIcon);
const normalizedSize = computed(() => (typeof props.size === "number" ? `${props.size}px` : props.size));
const ariaHidden = computed(() => (props.decorative ? "true" : undefined));
const role = computed(() => (props.decorative ? undefined : "img"));
</script>

<template>
  <svg
    class="app-icon"
    :data-icon-name="icons[props.name] ? props.name : 'unknown'"
    :width="normalizedSize"
    :height="normalizedSize"
    :viewBox="iconDefinition.viewBox"
    :aria-hidden="ariaHidden"
    :role="role"
    :aria-label="props.decorative ? undefined : props.name"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    v-html="iconDefinition.body"
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
