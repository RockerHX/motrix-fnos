<script setup lang="ts">
import { computed } from "vue";
import AppMetricCard from "./AppMetricCard.vue";

type AppMetricItem = {
  label: string;
  value: string | number;
  detail?: string;
  note?: string;
  tone?: "default" | "success" | "warning" | "error";
};

const props = withDefaults(
  defineProps<{
    items: AppMetricItem[];
    desktopColumns?: number;
    mobileColumns?: number;
  }>(),
  {
    desktopColumns: 3,
    mobileColumns: 1,
  },
);

const gridStyle = computed(() => ({
  "--app-metric-grid-desktop-columns": String(props.desktopColumns),
  "--app-metric-grid-mobile-columns": String(props.mobileColumns),
}));
</script>

<template>
  <div class="app-metric-grid" :style="gridStyle">
    <AppMetricCard
      v-for="item in props.items"
      :key="item.label"
      :label="item.label"
      :value="item.value"
      :detail="item.detail"
      :note="item.note"
      :tone="item.tone"
    />
  </div>
</template>

<style scoped>
.app-metric-grid {
  display: grid;
  grid-template-columns: repeat(var(--app-metric-grid-desktop-columns), minmax(0, 1fr));
  gap: 12px;
}

@media (max-width: 767px) {
  .app-metric-grid {
    grid-template-columns: repeat(var(--app-metric-grid-mobile-columns), minmax(0, 1fr));
    gap: 10px;
  }
}
</style>
