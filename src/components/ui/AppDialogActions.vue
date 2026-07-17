<script setup lang="ts">
import { NSpace } from "naive-ui";
</script>

<template>
  <NSpace class="app-dialog-actions w-full lt-md:flex-col-reverse" justify="end">
    <slot />
  </NSpace>
</template>

<style scoped src="./AppDialogActions.css"></style>
