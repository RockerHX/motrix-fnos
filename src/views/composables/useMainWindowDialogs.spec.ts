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

  it("switches between the RPC guide and settings without overlapping dialogs", () => {
    const dialogs = useMainWindowDialogs({
      taskStore: { isRuntimeExiting: false } as never,
      toolbar: { canCreate: ref(true) } as never,
      message: { warning: vi.fn() },
      t: (key) => key,
    });

    dialogs.openJsonRpcGuide();
    expect(dialogs.showJsonRpcGuide.value).toBe(true);
    expect(dialogs.showAbout.value).toBe(false);
    expect(dialogs.showSettings.value).toBe(false);

    dialogs.showCreateDialog.value = true;

    dialogs.openSettingsFromJsonRpcGuide();
    expect(dialogs.showJsonRpcGuide.value).toBe(false);
    expect(dialogs.showCreateDialog.value).toBe(false);
    expect(dialogs.showSettings.value).toBe(true);
    expect(dialogs.showAbout.value).toBe(false);
  });
});
