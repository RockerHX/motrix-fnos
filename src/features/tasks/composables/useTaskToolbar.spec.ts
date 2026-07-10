import { describe, expect, it } from "vitest";
import { ref } from "vue";
import { useTaskToolbar } from "./useTaskToolbar";
import type { MainNavCategory } from "../../../types/navigation";

describe("useTaskToolbar", () => {
  it("enables create only for task categories while runtime is active", () => {
    const activeCategory = ref<MainNavCategory>("downloading");
    const isRuntimeExiting = ref(false);
    const toolbar = useTaskToolbar({ activeCategory, isRuntimeExiting });

    expect(toolbar.canCreate.value).toBe(true);

    activeCategory.value = "trash";
    expect(toolbar.canCreate.value).toBe(false);

    activeCategory.value = "completed";
    expect(toolbar.canCreate.value).toBe(true);

    isRuntimeExiting.value = true;
    expect(toolbar.canCreate.value).toBe(false);
  });

  it("enables refresh outside extensions while runtime is active", () => {
    const activeCategory = ref<MainNavCategory>("trash");
    const isRuntimeExiting = ref(false);
    const toolbar = useTaskToolbar({ activeCategory, isRuntimeExiting });

    expect(toolbar.canRefresh.value).toBe(true);

    activeCategory.value = "extensions";
    expect(toolbar.canRefresh.value).toBe(false);

    activeCategory.value = "downloading";
    isRuntimeExiting.value = true;
    expect(toolbar.canRefresh.value).toBe(false);
  });
});
