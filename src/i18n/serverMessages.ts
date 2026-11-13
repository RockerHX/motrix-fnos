import { language, t, type TranslationKey, type TranslationParams } from ".";
import { enUS } from "./locales/en-US";

export function formatServerMessage(message: string, code?: string | null, params: TranslationParams = {}) {
  if (language.value === "zh-CN") return message;
  const marker = /^([a-z][a-z0-9_]+)[：:]/u.exec(message)?.[1];
  for (const candidate of [marker, code]) {
    const key = `server.${candidate}` as TranslationKey;
    if (candidate && Object.prototype.hasOwnProperty.call(enUS, key)) return t(key, params);
  }
  // Keep external diagnostics and user paths intact; unknown server codes remain identifiable.
  return code && /\p{Script=Han}/u.test(message) ? t("server.unknownCode", { code }) : message;
}
