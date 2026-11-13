import { computed } from "vue";
import changelogMarkdown from "../../../../CHANGELOG.md?raw";
import englishChangelogMarkdown from "../locales/CHANGELOG.en-US.md?raw";
import { language, t } from "../../../i18n";
import type { ChangelogEntry, ChangelogSection } from "../types";

const chineseEntries = parseChangelog(changelogMarkdown).slice(0, 5);
const englishEntries = parseChangelog(englishChangelogMarkdown, "Changes");
export const recentChangelogEntries = computed(() => language.value === "zh-CN" ? chineseEntries : chineseEntries.map((entry) => ({
  ...entry,
  sections: englishEntries.find(({ version }) => version === entry.version)?.sections ?? [{
    title: t("about.changelog.translationPending"),
    items: [t("about.changelog.translationPendingBody")],
  }],
})));

export function parseChangelog(markdown: string, defaultSectionTitle = "变更"): ChangelogEntry[] {
  const entries: ChangelogEntry[] = [];
  let currentEntry: ChangelogEntry | null = null;
  let currentSection: ChangelogSection | null = null;

  for (const rawLine of markdown.split(/\r?\n/)) {
    const line = rawLine.trim();
    const versionMatch = line.match(/^##\s+(.+?)(?:\s+-\s+(.+))?$/);
    if (versionMatch) {
      currentEntry = {
        version: versionMatch[1].trim(),
        date: (versionMatch[2] || "").trim(),
        sections: [],
      };
      entries.push(currentEntry);
      currentSection = null;
      continue;
    }

    const sectionMatch = line.match(/^###\s+(.+)$/);
    if (sectionMatch && currentEntry) {
      currentSection = {
        title: sectionMatch[1].trim(),
        items: [],
      };
      currentEntry.sections.push(currentSection);
      continue;
    }

    if (line.startsWith("- ") && currentEntry) {
      if (!currentSection) {
        currentSection = {
          title: defaultSectionTitle,
          items: [],
        };
        currentEntry.sections.push(currentSection);
      }
      currentSection.items.push(line.slice(2).trim());
    }
  }

  return entries.filter((entry) => entry.sections.some((section) => section.items.length > 0));
}
