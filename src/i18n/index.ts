import { computed, ref } from "vue";
import { zhCN, type TranslationKey } from "./locales/zh-CN";
import { enUS } from "./locales/en-US";

export const supportedLanguages = ["zh-CN", "en-US"] as const;
export type AppLanguage = (typeof supportedLanguages)[number];
export type TranslationParams = Record<string, string | number>;

const defaultLanguage: AppLanguage = "zh-CN";
const currentLanguage = ref<AppLanguage>(defaultLanguage);



const dictionaries = {
  "zh-CN": zhCN,
  "en-US": enUS,
} satisfies Record<AppLanguage, Record<keyof typeof zhCN, string>>;

export type { TranslationKey } from "./locales/zh-CN";

export const language = computed(() => currentLanguage.value);

export function normalizeLanguage(value: unknown): AppLanguage {
  return supportedLanguages.includes(value as AppLanguage) ? (value as AppLanguage) : defaultLanguage;
}

export function setLanguage(value: unknown) {
  currentLanguage.value = normalizeLanguage(value);
}

export function t(key: TranslationKey, params: TranslationParams = {}) {
  let value = dictionaries[currentLanguage.value][key] ?? zhCN[key] ?? key;
  for (const [name, replacement] of Object.entries(params)) {
    value = value.split(`{${name}}`).join(String(replacement));
  }
  return value;
}

export function useI18n() {
  return {
    language,
    setLanguage,
    t,
  };
}

export function formatDateTime(timestamp: number) {
  if (!timestamp) {
    return t("common.notAvailable");
  }
  return new Date(timestamp).toLocaleString(currentLanguage.value);
}
