<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    label: string;
    value: string | number;
    detail?: string;
    note?: string;
    tone?: "default" | "success" | "warning" | "error";
  }>(),
  {
    detail: "",
    note: "",
    tone: "default",
  },
);
</script>

<template>
  <div class="app-metric-card" :class="`app-metric-card--${props.tone}`">
    <span class="app-metric-label"><slot name="label">{{ props.label }}</slot></span>
    <strong class="app-metric-value"><slot name="value">{{ props.value }}</slot></strong>
    <p v-if="props.detail || $slots.detail" class="app-metric-detail"><slot name="detail">{{ props.detail }}</slot></p>
    <small v-if="props.note || $slots.note" class="app-metric-note"><slot name="note">{{ props.note }}</slot></small>
  </div>
</template>

<style scoped>
.app-metric-card {
  min-width: 0;
  padding: 14px;
  border: 1px solid var(--app-color-border-subtle);
  border-radius: var(--app-radius-md);
  background: var(--app-color-card-overlay-subtle);
}

.app-metric-card--success {
  border-color: color-mix(in srgb, var(--app-text-accent-soft) 34%, var(--app-color-border-subtle));
}

.app-metric-card--warning {
  border-color: color-mix(in srgb, #f2c97d 36%, var(--app-color-border-subtle));
}

.app-metric-card--error {
  border-color: color-mix(in srgb, var(--app-text-danger) 40%, var(--app-color-border-subtle));
}

.app-metric-label,
.app-metric-note {
  color: var(--app-text-dim);
  font-size: 12px;
}

.app-metric-label {
  display: block;
}

.app-metric-value {
  display: block;
  overflow: hidden;
  margin: 8px 0;
  color: var(--app-text-strong);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.app-metric-detail {
  overflow: hidden;
  margin: 0 0 8px;
  color: var(--app-text-secondary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.app-metric-note {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
