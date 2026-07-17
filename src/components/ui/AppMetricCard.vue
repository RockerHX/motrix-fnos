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
  <div class="app-metric-card min-w-0" :class="`app-metric-card--${props.tone}`">
    <span class="app-metric-label block"><slot name="label">{{ props.label }}</slot></span>
    <strong class="app-metric-value block my-2 truncate"><slot name="value">{{ props.value }}</slot></strong>
    <p v-if="props.detail || $slots.detail" class="app-metric-detail m-0 mb-2 truncate"><slot name="detail">{{ props.detail }}</slot></p>
    <small v-if="props.note || $slots.note" class="app-metric-note block truncate"><slot name="note">{{ props.note }}</slot></small>
  </div>
</template>

<style scoped src="./AppMetricCard.css"></style>
