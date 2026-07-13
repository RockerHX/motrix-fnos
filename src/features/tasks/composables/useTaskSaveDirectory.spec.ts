import { createPinia, setActivePinia } from "pinia";
import { reactive } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getAccessiblePaths } from "../../../services/storage";
import { getAppConfig } from "../../../services/settings";
import { createTaskCreateFormState } from "./taskCreateFormModel";
import { useTaskSaveDirectory } from "./useTaskSaveDirectory";

vi.mock("../../../services/storage", () => ({ getAccessiblePaths: vi.fn() }));
vi.mock("../../../services/settings", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../../services/settings")>()),
  getAppConfig: vi.fn(),
}));

const mockedGetAccessiblePaths = vi.mocked(getAccessiblePaths);
const mockedGetAppConfig = vi.mocked(getAppConfig);

describe("useTaskSaveDirectory", () => {
  beforeEach(() => {
    localStorage.clear();
    setActivePinia(createPinia());
    mockedGetAccessiblePaths.mockResolvedValue({ paths: ["/downloads", "/backup"] });
    mockedGetAppConfig.mockResolvedValue(config("/downloads"));
  });

  it("prefers the configured default, then remembered path, then first authorized path", async () => {
    const form = reactive(createTaskCreateFormState());
    const saveDirectory = useTaskSaveDirectory(form);
    await saveDirectory.refreshAccessiblePaths();
    expect(form.saveDir).toBe("/downloads");

    localStorage.setItem("motrix-fnos:last-save-dir", "/backup");
    mockedGetAppConfig.mockResolvedValueOnce(config("/missing"));
    form.saveDir = "";
    await saveDirectory.refreshAccessiblePaths();
    expect(form.saveDir).toBe("/backup");

    localStorage.clear();
    form.saveDir = "";
    await saveDirectory.refreshAccessiblePaths();
    expect(form.saveDir).toBe("/downloads");
  });

  it("clears selection and exposes loading errors", async () => {
    mockedGetAccessiblePaths.mockRejectedValueOnce(new Error("load failed"));
    const form = reactive(createTaskCreateFormState());
    form.saveDir = "/old";
    const saveDirectory = useTaskSaveDirectory(form);

    await saveDirectory.refreshAccessiblePaths();

    expect(form.saveDir).toBe("");
    expect(saveDirectory.accessiblePaths.value).toEqual([]);
    expect(saveDirectory.accessiblePathsError.value).toBe("load failed");
  });
});

function config(defaultDownloadDir: string) {
  return {
    defaultDownloadDir,
    maxConcurrentDownloads: 3,
    downloadLimit: 0,
    uploadLimit: 0,
    language: "zh-CN" as const,
    jsonRpcToken: "",
  };
}
