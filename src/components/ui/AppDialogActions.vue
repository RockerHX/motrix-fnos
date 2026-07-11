<script setup lang="ts">
import { NSpace } from "naive-ui";
</script>

<template>
  <NSpace class="app-dialog-actions" justify="end">
    <slot />
  </NSpace>
</template>

<style scoped>
.app-dialog-actions {
  width: 100%;
}

@media (max-width: 767px) {
  .app-dialog-actions {
    flex-direction: column-reverse;
  }

  .app-dialog-actions :deep(.n-button) {
    width: 100%;
  }
}
</style>
