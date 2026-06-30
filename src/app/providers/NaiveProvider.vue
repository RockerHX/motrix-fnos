<script setup lang="ts">
import { darkTheme, NConfigProvider, NDialogProvider, NMessageProvider, NNotificationProvider } from "naive-ui";
import type { GlobalThemeOverrides } from "naive-ui";

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
</script>

<template>
  <NConfigProvider :theme="darkTheme" :theme-overrides="themeOverrides">
    <NMessageProvider placement="top-right">
      <NDialogProvider>
        <NNotificationProvider placement="top-right">
          <slot />
        </NNotificationProvider>
      </NDialogProvider>
    </NMessageProvider>
  </NConfigProvider>
</template>
