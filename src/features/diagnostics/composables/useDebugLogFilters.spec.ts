import { ref } from "vue";
import { describe, expect, it } from "vitest";
import type { DebugLogEntry } from "../types";
import { useDebugLogFilters } from "./useDebugLogFilters";

const logs: DebugLogEntry[] = [
  { id: 1, timestampMs: 1, lastTimestampMs: 1, level: "info", category: "app", module: "app", message: "started", repeatCount: 1 },
  { id: 2, timestampMs: 2, lastTimestampMs: 2, level: "warn", category: "aria2", module: "aria2.rpc", message: "retry", repeatCount: 3 },
  { id: 3, timestampMs: 3, lastTimestampMs: 3, level: "error", category: "aria2", module: "aria2.rpc", message: "failed", repeatCount: 1 },
];

describe("useDebugLogFilters", () => {
  it("filters by problem level, category, module and keyword", () => {
    const filters = useDebugLogFilters(ref(logs));
    filters.onlyProblems.value = true;
    expect(filters.filteredLogs.value.map((log) => log.id)).toEqual([2, 3]);
    filters.levelFilter.value = "error";
    expect(filters.filteredLogs.value.map((log) => log.id)).toEqual([3]);
    filters.levelFilter.value = null;
    filters.categoryFilter.value = "aria2";
    filters.moduleFilter.value = "aria2.rpc";
    filters.searchText.value = "retry";
    expect(filters.filteredLogs.value.map((log) => log.id)).toEqual([2]);
  });

  it("counts levels and repeated module entries, then clears filters", () => {
    const filters = useDebugLogFilters(ref(logs));
    expect(filters.logStats.value).toEqual({ total: 3, filtered: 3, errors: 1, warnings: 1, topModule: "aria2.rpc" });
    filters.onlyProblems.value = true;
    filters.searchText.value = "failed";
    filters.clearFilters();
    expect(filters.onlyProblems.value).toBe(false);
    expect(filters.searchText.value).toBe("");
    expect(filters.filteredLogs.value).toHaveLength(3);
  });
});
