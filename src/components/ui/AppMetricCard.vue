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

<style scoped src="./AppMetricCard.css"></style>
