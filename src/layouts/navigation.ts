import type { TranslationKey } from "../i18n";
import type { MainNavCategory } from "../types/navigation";

export type MainNavItemDefinition = {
  key: MainNavCategory;
  icon: string;
  labelKey: TranslationKey;
  spaced?: boolean;
};

export const mainNavItems: MainNavItemDefinition[] = [
  { key: "downloading", icon: "⇩", labelKey: "nav.downloading" },
  { key: "completed", icon: "✓", labelKey: "nav.completed" },
  { key: "stopped", icon: "Ⅱ", labelKey: "nav.stopped" },
  { key: "trash", icon: "♜", labelKey: "nav.trash", spaced: true },
  { key: "extensions", icon: "♧", labelKey: "nav.extensions" },
];

export function getMainNavLabelKey(category: MainNavCategory): TranslationKey {
  return mainNavItems.find((item) => item.key === category)?.labelKey ?? "nav.downloading";
}
