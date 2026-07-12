<script setup lang="ts">
import {
  darkTheme,
  dateEnUS,
  dateZhCN,
  enUS,
  NConfigProvider,
  NDialogProvider,
  NMessageProvider,
  NNotificationProvider,
  zhCN,
} from "naive-ui";
import { computed } from "vue";
import type { GlobalThemeOverrides } from "naive-ui";
import { language } from "../../i18n";

const themeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: "#68ae5a",
    primaryColorHover: "#7bc96d",
    primaryColorPressed: "#57964b",
    primaryColorSuppl: "#8ef08a",
    borderRadius: "7px",
    bodyColor: "#151515",
    cardColor: "#1b1f1d",
    modalColor: "#1b1f1d",
    popoverColor: "#1b1f1d",
    tableColor: "#151515",
    tableHeaderColor: "#202720",
    textColorBase: "#d7dfd8",
  },
};

const naiveLocale = computed(() => (language.value === "en-US" ? enUS : zhCN));
const naiveDateLocale = computed(() => (language.value === "en-US" ? dateEnUS : dateZhCN));
</script>

<template>
  <NConfigProvider :theme="darkTheme" :theme-overrides="themeOverrides" :locale="naiveLocale" :date-locale="naiveDateLocale">
    <NMessageProvider placement="top">
      <NDialogProvider>
        <NNotificationProvider placement="bottom-right">
          <slot />
        </NNotificationProvider>
      </NDialogProvider>
    </NMessageProvider>
  </NConfigProvider>
</template>
