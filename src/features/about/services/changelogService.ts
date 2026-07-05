import changelogMarkdown from "../../../../CHANGELOG.md?raw";
import type { ChangelogEntry, ChangelogSection } from "../types";

export const recentChangelogEntries = parseChangelog(changelogMarkdown).slice(0, 5);

export function parseChangelog(markdown: string): ChangelogEntry[] {
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

    if (line.startsWith("- ") && currentSection) {
      currentSection.items.push(line.slice(2).trim());
    }
  }

  return entries.filter((entry) => entry.sections.some((section) => section.items.length > 0));
}
