import { readonly, ref } from "vue";
import type { FnosTheme } from "../services/fnos";

const currentTheme = ref<FnosTheme>("dark");

export const appTheme = readonly(currentTheme);

export function setAppTheme(theme: FnosTheme) {
  currentTheme.value = theme;
  if (typeof document !== "undefined") {
    document.documentElement.dataset.theme = theme;
    document.documentElement.style.colorScheme = theme;
  }
}

setAppTheme("dark");
