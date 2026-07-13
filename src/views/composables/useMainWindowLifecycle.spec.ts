import { defineComponent, h, nextTick, ref } from "vue";
import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import { useMainWindowLifecycle } from "./useMainWindowLifecycle";

describe("useMainWindowLifecycle", () => {
  it("refreshes on mount, reports errors and closes create while exiting", async () => {
    const errorMessage = ref("");
    const isRuntimeExiting = ref(false);
    const showCreateDialog = ref(true);
    const message = { error: vi.fn() };
    const refreshTasks = vi.fn().mockResolvedValue(undefined);
    const refreshAria2Status = vi.fn().mockResolvedValue(undefined);

    mount(defineComponent({
      setup() {
        useMainWindowLifecycle({ errorMessage, isRuntimeExiting, showCreateDialog, message, refreshTasks, refreshAria2Status });
        return () => h("div");
      },
    }));

    expect(refreshTasks).toHaveBeenCalledWith(true);
    expect(refreshAria2Status).toHaveBeenCalled();
    errorMessage.value = "backend failed";
    isRuntimeExiting.value = true;
    await nextTick();
    expect(message.error).toHaveBeenCalledWith("backend failed");
    expect(showCreateDialog.value).toBe(false);
  });
});
