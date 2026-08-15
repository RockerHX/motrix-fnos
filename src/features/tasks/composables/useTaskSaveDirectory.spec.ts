import { createPinia, setActivePinia } from "pinia";
import { reactive } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fnosHost } from "../../../services/fnos";
import { getAccessiblePaths, refreshAccessiblePaths } from "../../../services/storage";
import { getAppConfig } from "../../../services/settings";
import { createTaskCreateFormState } from "./taskCreateFormModel";
import { useTaskSaveDirectory } from "./useTaskSaveDirectory";

vi.mock("../../../services/storage", () => ({
  getAccessiblePaths: vi.fn(),
  refreshAccessiblePaths: vi.fn(),
}));
vi.mock("../../../services/fnos", () => ({
  fnosHost: {
    getHostKind: vi.fn(),
    requestSharedFolderAuthorization: vi.fn(),
  },
}));
vi.mock("../../../services/settings", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../../services/settings")>()),
  getAppConfig: vi.fn(),
}));

const mockedGetAccessiblePaths = vi.mocked(getAccessiblePaths);
const mockedRefreshAccessiblePaths = vi.mocked(refreshAccessiblePaths);
const mockedGetAppConfig = vi.mocked(getAppConfig);

describe("useTaskSaveDirectory", () => {
  beforeEach(() => {
    localStorage.clear();
    setActivePinia(createPinia());
    mockedGetAccessiblePaths.mockResolvedValue({ paths: ["/downloads", "/backup"] });
    mockedRefreshAccessiblePaths.mockResolvedValue({ paths: ["/downloads", "/backup"] });
    mockedGetAppConfig.mockResolvedValue(config("/downloads"));
    vi.mocked(fnosHost.getHostKind).mockResolvedValue("hosted");
    vi.mocked(fnosHost.requestSharedFolderAuthorization).mockResolvedValue({ status: "authorized" });
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

  it("queries the official API after a successful picker result and selects the confirmed path", async () => {
    mockedRefreshAccessiblePaths.mockResolvedValueOnce({ paths: ["/new-downloads"] });
    mockedGetAppConfig.mockResolvedValueOnce(config("/new-downloads"));
    const form = reactive(createTaskCreateFormState());
    const saveDirectory = useTaskSaveDirectory(form);
    await saveDirectory.detectHostKind();

    await saveDirectory.addAccessiblePath();

    expect(fnosHost.requestSharedFolderAuthorization).toHaveBeenCalledOnce();
    expect(mockedRefreshAccessiblePaths).toHaveBeenCalledOnce();
    expect(form.saveDir).toBe("/new-downloads");
  });

  it.each([
    ["cancelled", { status: "cancelled" }],
    ["admin_required", { status: "admin_required" }],
  ] as const)("does not load or select a path after picker %s", async (_name, result) => {
    vi.mocked(fnosHost.requestSharedFolderAuthorization).mockResolvedValueOnce(result);
    const form = reactive(createTaskCreateFormState());
    const saveDirectory = useTaskSaveDirectory(form);
    await saveDirectory.detectHostKind();

    await saveDirectory.addAccessiblePath();

    expect(mockedRefreshAccessiblePaths).not.toHaveBeenCalled();
    expect(form.saveDir).toBe("");
  });

  it("clears the unconfirmed selection when the official refresh fails", async () => {
    mockedRefreshAccessiblePaths.mockRejectedValueOnce(new Error("上游刷新失败"));
    const form = reactive(createTaskCreateFormState());
    form.saveDir = "/old";
    const saveDirectory = useTaskSaveDirectory(form);
    await saveDirectory.detectHostKind();

    await saveDirectory.addAccessiblePath();

    expect(form.saveDir).toBe("");
    expect(saveDirectory.accessiblePaths.value).toEqual([]);
    expect(saveDirectory.authorizationMessage.value).toBe("当前目录列表可能已过期，仍保留上一次确认的目录。");
  });
});

function config(defaultDownloadDir: string) {
  return {
    defaultDownloadDir,
    maxConcurrentDownloads: 3,
    downloadLimit: 0,
    uploadLimit: 0,
    language: "zh-CN" as const,
  };
}
