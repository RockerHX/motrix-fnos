<script setup lang="ts">
import { useI18n } from "../../../i18n";

const { t } = useI18n();
</script>

<template>
  <section class="extensions-page">
    <div class="extensions-card">
      <span class="extensions-icon" aria-hidden="true">♧</span>
      <p class="eyebrow">{{ t("extensions.eyebrow") }}</p>
      <h1>{{ t("extensions.title") }}</h1>
      <p>{{ t("extensions.body") }}</p>
      <ul>
        <li>{{ t("extensions.item.noScripts") }}</li>
        <li>{{ t("extensions.item.noNetworkPlugins") }}</li>
        <li>{{ t("extensions.item.futureEntry") }}</li>
      </ul>
    </div>
  </section>
</template>

<style scoped src="./ExtensionsPlaceholder.css"></style>
