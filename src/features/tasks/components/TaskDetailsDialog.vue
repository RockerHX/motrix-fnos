<script setup lang="ts">
import { NButton, NCard, NDescriptions, NDescriptionsItem, NModal, NSpace } from "naive-ui";
import type { TaskActionDetails } from "./taskActionViewModel";

defineProps<{
  show: boolean;
  details: TaskActionDetails;
  closeLabel: string;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
}>();
</script>

<template>
  <NModal :show="show" @update:show="emit('update:show', $event)">
    <NCard class="task-detail-card app-dialog" role="dialog" aria-modal="true" :title="details.title">
      <NDescriptions :column="1" label-placement="left" bordered>
        <NDescriptionsItem v-for="item in details.items" :key="item.label" :label="item.label">
          {{ item.value }}
        </NDescriptionsItem>
      </NDescriptions>

      <template #footer>
        <NSpace justify="end">
          <NButton @click="emit('update:show', false)">{{ closeLabel }}</NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>
</template>

<style scoped src="./TaskDetailsDialog.css"></style>
