import { computed, ref, watch } from "vue";
import { zhCN, type TranslationKey } from "./locales/zh-CN";
import { enUS } from "./locales/en-US";

export const supportedLanguages = ["zh-CN", "en-US"] as const;
export type AppLanguage = (typeof supportedLanguages)[number];
export type AppLanguagePreference = AppLanguage | "auto";
export type TranslationParams = Record<string, string | number>;
export const LANGUAGE_PREFERENCE_KEY = "motrix-fnos:language";

const hostLanguage = ref(normalizeLanguage(typeof navigator === "undefined" ? null : navigator.language));
const currentPreference = ref<AppLanguagePreference>(getLocalLanguagePreference() ?? "auto");
const dictionaries = {
  "zh-CN": zhCN,
  "en-US": enUS,
} satisfies Record<AppLanguage, Record<keyof typeof zhCN, string>>;

export type { TranslationKey } from "./locales/zh-CN";

export const languagePreference = computed(() => currentPreference.value);
export const language = computed(() => currentPreference.value === "auto" ? hostLanguage.value : currentPreference.value);

watch(language, (value) => {
  if (typeof document !== "undefined") document.documentElement.lang = value;
}, { immediate: true });

export function normalizeLanguage(value: unknown): AppLanguage {
  return typeof value === "string" && /^zh(?:[-_]|$)/i.test(value.trim()) ? "zh-CN" : "en-US";
}

export function normalizeLanguagePreference(value: unknown): AppLanguagePreference {
  return supportedLanguages.includes(value as AppLanguage) ? (value as AppLanguage) : "auto";
}

export function setLanguage(value: unknown) {
  currentPreference.value = normalizeLanguagePreference(value);
}

export function setHostLanguage(value: string) {
  hostLanguage.value = normalizeLanguage(value);
}

export function getLocalLanguagePreference(): AppLanguage | null {
  try {
    const value = localStorage.getItem(LANGUAGE_PREFERENCE_KEY);
    return supportedLanguages.includes(value as AppLanguage) ? (value as AppLanguage) : null;
  } catch {
    return null;
  }
}

export function saveLocalLanguagePreference(value: AppLanguagePreference) {
  try {
    if (value === "auto") localStorage.removeItem(LANGUAGE_PREFERENCE_KEY);
    else localStorage.setItem(LANGUAGE_PREFERENCE_KEY, value);
  } catch {
    // 无本地存储时仍允许在当前页面切换语言。
  }
}

export function t(key: TranslationKey, params: TranslationParams = {}) {
  let value = dictionaries[language.value][key] ?? enUS[key] ?? key;
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
  return new Date(timestamp).toLocaleString(language.value);
}
