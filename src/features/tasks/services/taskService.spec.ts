import { beforeEach, describe, expect, it, vi } from "vitest";
import { httpDelete, httpGet, httpPost, httpPostFormData, httpPut } from "../../../services/http";
import {
  confirmDownloadTaskFiles,
  createBatchDownloadTasks,
  createDownloadTask,
  createTorrentDownloadTask,
  deleteDownloadTask,
  getTaskFileContext,
  listDownloadTasks,
  listRemovedDownloadTasks,
  pauseDownloadTask,
  permanentlyDeleteDownloadTask,
  redownloadDownloadTask,
  resumeDownloadTask,
  restoreDownloadTask,
  updateDownloadTaskProxy,
} from "./taskService";

vi.mock("../../../services/http", () => ({
  httpDelete: vi.fn(),
  httpGet: vi.fn(),
  httpPost: vi.fn(),
  httpPostFormData: vi.fn(),
  httpPut: vi.fn(),
}));

describe("taskService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("routes create and list requests through the expected endpoints", async () => {
    const createPayload = { url: "https://example.com/a.iso", saveDir: "/downloads" };
    const batchPayload = { urls: [createPayload.url], saveDir: "/downloads" };

    createDownloadTask(createPayload);
    createBatchDownloadTasks(batchPayload);
    listDownloadTasks();
    listRemovedDownloadTasks();

    expect(httpPost).toHaveBeenNthCalledWith(1, "/api/tasks", createPayload);
    expect(httpPost).toHaveBeenNthCalledWith(2, "/api/tasks/batch", batchPayload);
    expect(httpGet).toHaveBeenNthCalledWith(1, "/api/tasks");
    expect(httpGet).toHaveBeenNthCalledWith(2, "/api/tasks?status=removed");
  });

  it("serializes torrent metadata into FormData", () => {
    const torrent = new File(["torrent-data"], "movie.torrent", { type: "application/x-bittorrent" });

    createTorrentDownloadTask({
      torrent,
      saveDir: "/downloads",
      startMode: "paused",
      category: "电影",
      advancedOptions: { connections: 8, downloadLimitKb: 512, proxy: null },
    });

    expect(httpPostFormData).toHaveBeenCalledOnce();
    const [path, formData] = vi.mocked(httpPostFormData).mock.calls[0];
    expect(path).toBe("/api/tasks/torrent");
    expect(formData.get("torrent")).toBe(torrent);
    expect(JSON.parse(String(formData.get("request")))).toEqual({
      saveDir: "/downloads",
      startMode: "paused",
      category: "电影",
      advancedOptions: { connections: 8, downloadLimitKb: 512, proxy: null },
    });
  });

  it("routes task controls and confirmation payloads", () => {
    pauseDownloadTask(7);
    resumeDownloadTask(7);
    confirmDownloadTaskFiles(7, { selectedFileIndexes: [3, 1] });
    updateDownloadTaskProxy(7, true);
    redownloadDownloadTask(7, false);
    restoreDownloadTask(8, true);
    redownloadDownloadTask(9);

    expect(httpPost).toHaveBeenNthCalledWith(1, "/api/tasks/7/pause");
    expect(httpPost).toHaveBeenNthCalledWith(2, "/api/tasks/7/resume");
    expect(httpPost).toHaveBeenNthCalledWith(3, "/api/tasks/7/confirm", { selectedFileIndexes: [3, 1] });
    expect(httpPut).toHaveBeenCalledWith("/api/tasks/7/proxy", { enabled: true });
    expect(httpPost).toHaveBeenNthCalledWith(4, "/api/tasks/7/redownload", { useProxy: false });
    expect(httpPost).toHaveBeenNthCalledWith(5, "/api/tasks/8/restore", { useProxy: true });
    expect(httpPost).toHaveBeenNthCalledWith(6, "/api/tasks/9/redownload");
  });

  it("encodes soft and permanent delete endpoints", () => {
    deleteDownloadTask(9, true);
    deleteDownloadTask(10, false);
    permanentlyDeleteDownloadTask(11);

    expect(httpDelete).toHaveBeenNthCalledWith(1, "/api/tasks/9?deleteFiles=true");
    expect(httpDelete).toHaveBeenNthCalledWith(2, "/api/tasks/10?deleteFiles=false");
    expect(httpDelete).toHaveBeenNthCalledWith(3, "/api/tasks/11/permanent");
  });

  it("requests task file context with the selected language", () => {
    getTaskFileContext(42, "zh-CN");

    expect(httpGet).toHaveBeenCalledWith("/api/tasks/42/file-context?language=zh-CN");
  });
});
