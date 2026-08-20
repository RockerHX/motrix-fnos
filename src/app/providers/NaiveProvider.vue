<script setup lang="ts">
import {
  darkTheme,
  dateEnUS,
  dateZhCN,
  enUS,
  NConfigProvider,
  NDialogProvider,
  NGlobalStyle,
  NMessageProvider,
  NNotificationProvider,
  zhCN,
} from "naive-ui";
import { computed } from "vue";
import type { GlobalThemeOverrides } from "naive-ui";
import { language } from "../../i18n";
import { appTheme } from "../theme";

const darkThemeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: "#3374db",
    primaryColorHover: "#5da9ff",
    primaryColorPressed: "#285bae",
    primaryColorSuppl: "#a8c8f0",
    borderRadius: "7px",
    bodyColor: "#0b0f0e",
    cardColor: "#151515",
    modalColor: "#151515",
    popoverColor: "#1a2331",
    tableColor: "#151515",
    tableHeaderColor: "#1a2331",
    textColorBase: "#dce8e2",
    borderColor: "rgba(255, 255, 255, 0.08)",
  },
};

const lightThemeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: "#285fbc",
    primaryColorHover: "#3374db",
    primaryColorPressed: "#204d98",
    primaryColorSuppl: "#3374db",
    borderRadius: "7px",
    bodyColor: "#f5f8fc",
    cardColor: "#ffffff",
    modalColor: "#ffffff",
    popoverColor: "#ffffff",
    tableColor: "#ffffff",
    tableHeaderColor: "#edf3fa",
    textColorBase: "#142236",
    borderColor: "#d6e0ec",
  },
};

const naiveLocale = computed(() => (language.value === "en-US" ? enUS : zhCN));
const naiveDateLocale = computed(() => (language.value === "en-US" ? dateEnUS : dateZhCN));
const naiveTheme = computed(() => (appTheme.value === "dark" ? darkTheme : null));
const themeOverrides = computed(() =>
  appTheme.value === "dark" ? darkThemeOverrides : lightThemeOverrides,
);
</script>

<template>
  <NConfigProvider :theme="naiveTheme" :theme-overrides="themeOverrides" :locale="naiveLocale" :date-locale="naiveDateLocale">
    <NGlobalStyle />
    <NMessageProvider placement="top">
      <NDialogProvider>
        <NNotificationProvider placement="bottom-right">
          <slot />
        </NNotificationProvider>
      </NDialogProvider>
    </NMessageProvider>
  </NConfigProvider>
</template>
