import { describe, expect, it, vi } from "vitest";
import { ref } from "vue";
import { useMainWindowDialogs } from "./useMainWindowDialogs";

describe("useMainWindowDialogs", () => {
  it("opens dialog state and rejects task creation while exiting", () => {
    const taskStore = { isRuntimeExiting: false };
    const message = { warning: vi.fn() };
    const dialogs = useMainWindowDialogs({
      taskStore: taskStore as never,
      toolbar: { canCreate: ref(true) } as never,
      message,
      t: (key) => key,
    });
    dialogs.openCreateDialog();
    expect(dialogs.showCreateDialog.value).toBe(true);

    dialogs.showCreateDialog.value = false;
    taskStore.isRuntimeExiting = true;
    dialogs.openCreateDialog();
    expect(dialogs.showCreateDialog.value).toBe(false);
    expect(message.warning).toHaveBeenCalledWith("task.runtimeExiting");
  });
});
