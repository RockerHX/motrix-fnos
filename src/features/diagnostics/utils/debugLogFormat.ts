import { t, type TranslationKey } from "../../../i18n";
import type { DebugLogCategory } from "../types";

const categoryKeys: Record<DebugLogCategory, TranslationKey> = {
  app: "logs.category.app",
  task: "logs.category.task",
  aria2: "logs.category.aria2",
  settings: "logs.category.settings",
  storage: "logs.category.storage",
  api: "logs.category.api",
  runtime: "logs.category.runtime",
};

export function formatDebugLogCategory(category: string) {
  return Object.prototype.hasOwnProperty.call(categoryKeys, category) ? t(categoryKeys[category as DebugLogCategory]) : category;
}
