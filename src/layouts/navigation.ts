import type { TranslationKey } from "../i18n";
import type { MainNavCategory } from "../types/navigation";

export type MainNavItemDefinition = {
  key: MainNavCategory;
  iconName: string;
  labelKey: TranslationKey;
  spaced?: boolean;
};

export const mainNavItems: MainNavItemDefinition[] = [
  { key: "downloading", iconName: "download", labelKey: "nav.downloading" },
  { key: "completed", iconName: "completed", labelKey: "nav.completed" },
  { key: "stopped", iconName: "pause", labelKey: "nav.stopped" },
  { key: "trash", iconName: "trash", labelKey: "nav.trash", spaced: true },
  { key: "extensions", iconName: "extensions", labelKey: "nav.extensions" },
];

export function getMainNavLabelKey(category: MainNavCategory): TranslationKey {
  return mainNavItems.find((item) => item.key === category)?.labelKey ?? "nav.downloading";
}
