import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DownloadProxyMutationResponse } from "../../../types/settings";
import {
  deleteDownloadProxy,
  getDownloadProxyStatus,
  updateDownloadProxy,
} from "../services/downloadProxyService";
import { useDownloadProxyStore } from "./downloadProxyStore";

vi.mock("../services/downloadProxyService", () => ({
  deleteDownloadProxy: vi.fn(),
  getDownloadProxyStatus: vi.fn(),
  updateDownloadProxy: vi.fn(),
}));

const mockedDelete = vi.mocked(deleteDownloadProxy);
const mockedGetStatus = vi.mocked(getDownloadProxyStatus);
const mockedUpdate = vi.mocked(updateDownloadProxy);

describe("downloadProxyStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("loads only the masked server status", async () => {
    mockedGetStatus.mockResolvedValueOnce({
      configured: true,
      maskedProxyUrl: "http://***:***@proxy.example.com:7890/",
      revision: 3,
    });
    const store = useDownloadProxyStore();

    await store.loadStatus();

    expect(store.status?.maskedProxyUrl).toBe("http://***:***@proxy.example.com:7890/");
    expect(store.draftProxyUrl).toBe("");
    expect(localStorage.length).toBe(0);
    expect(sessionStorage.length).toBe(0);
  });

  it("clears the raw draft as soon as saving starts", async () => {
    const store = useDownloadProxyStore();
    const deferred = deferredResponse();
    mockedUpdate.mockReturnValueOnce(deferred.promise);
    store.setDraftProxyUrl("http://user:password@proxy.example.com:7890");

    const saving = store.saveDraft();

    expect(mockedUpdate).toHaveBeenCalledWith("http://user:password@proxy.example.com:7890");
    expect(store.draftProxyUrl).toBe("");
    expect(store.isSaving).toBe(true);

    deferred.resolve(proxyMutationResponse());
    await saving;
    expect(store.status?.revision).toBe(4);
    expect(store.lastMutation?.appliedTaskIds).toEqual([1]);
    expect(store.isSaving).toBe(false);
  });

  it("keeps raw input cleared after a failed save", async () => {
    const store = useDownloadProxyStore();
    store.setDraftProxyUrl("socks5://proxy.example.com:1080");
    mockedUpdate.mockRejectedValueOnce(new Error("save failed"));

    await expect(store.saveDraft()).rejects.toThrow("save failed");

    expect(store.draftProxyUrl).toBe("");
    expect(store.isSaving).toBe(false);
  });

  it("does not restore transient results after the settings dialog closes", async () => {
    const store = useDownloadProxyStore();
    const deferred = deferredResponse();
    mockedUpdate.mockReturnValueOnce(deferred.promise);
    store.setDraftProxyUrl("http://proxy.example.com:7890");

    const saving = store.saveDraft();
    store.clearTransientState();
    deferred.resolve(proxyMutationResponse());
    await saving;

    expect(store.draftProxyUrl).toBe("");
    expect(store.lastMutation).toBeNull();
    expect(store.status?.configured).toBe(true);
  });

  it("clears the configuration without retaining a raw draft", async () => {
    const store = useDownloadProxyStore();
    store.status = proxyMutationResponse().status;
    store.setDraftProxyUrl("temporary-value");
    mockedDelete.mockResolvedValueOnce(undefined);

    await store.clearProxy();

    expect(mockedDelete).toHaveBeenCalledOnce();
    expect(store.status).toEqual({ configured: false, maskedProxyUrl: null, revision: 0 });
    expect(store.draftProxyUrl).toBe("");
  });
});

function proxyMutationResponse(): DownloadProxyMutationResponse {
  return {
    status: {
      configured: true,
      maskedProxyUrl: "http://***:***@proxy.example.com:7890/",
      revision: 4,
    },
    appliedTaskIds: [1],
    deferredTaskIds: [2],
    failed: [{ taskId: 3, code: "runtime_transition", message: "Aria2 正在切换运行状态，请稍后重试" }],
  };
}

function deferredResponse() {
  let resolve!: (response: DownloadProxyMutationResponse) => void;
  const promise = new Promise<DownloadProxyMutationResponse>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}
