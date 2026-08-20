<script setup lang="ts">
import { computed } from "vue";
import AppMetricCard from "./AppMetricCard.vue";
import type { AppMetricItem } from "./appMetric";

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
  <div class="app-metric-grid grid gap-3 mobile:gap-2.5" :style="gridStyle">
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

<style scoped src="./AppMetricGrid.css"></style>
