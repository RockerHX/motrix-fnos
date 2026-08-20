import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getJsonRpcTokenStatus, updateJsonRpcToken } from "../services/jsonRpcTokenService";
import { useJsonRpcTokenStore } from "./jsonRpcTokenStore";

vi.mock("../services/jsonRpcTokenService", () => ({ getJsonRpcTokenStatus: vi.fn(), updateJsonRpcToken: vi.fn() }));

const mockedGetStatus = vi.mocked(getJsonRpcTokenStatus);
const mockedUpdate = vi.mocked(updateJsonRpcToken);

describe("jsonRpcTokenStore", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("loads only masked status and generates 32 random bytes", async () => {
    const store = useJsonRpcTokenStore();
    mockedGetStatus.mockResolvedValueOnce({ configured: true, maskedToken: "••••••••abcd" });
    vi.spyOn(crypto, "getRandomValues").mockImplementation((array) => {
      (array as Uint8Array).fill(0xab);
      return array;
    });

    await store.loadStatus();
    const generated = store.generateToken();

    expect(store.status).toEqual({ configured: true, maskedToken: "••••••••abcd" });
    expect(generated).toBe("ab".repeat(32));
    expect(localStorage.length).toBe(0);
    expect(sessionStorage.length).toBe(0);
  });

  it("clears the raw draft immediately after saving and clearing", async () => {
    const store = useJsonRpcTokenStore();
    store.draftToken = "raw-token-value";
    mockedUpdate.mockResolvedValueOnce({ configured: true, maskedToken: "••••••••alue" });

    await store.saveDraft();

    expect(mockedUpdate).toHaveBeenCalledWith("raw-token-value");
    expect(store.draftToken).toBe("");
    expect(store.status).toEqual({ configured: true, maskedToken: "••••••••alue" });

    mockedUpdate.mockResolvedValueOnce({ configured: false, maskedToken: null });
    await store.clearToken();
    expect(mockedUpdate).toHaveBeenLastCalledWith("");
    expect(store.status).toEqual({ configured: false, maskedToken: null });
  });

  it("retains an unsaved draft after a failed save so the user can retry", async () => {
    const store = useJsonRpcTokenStore();
    store.draftToken = "unsaved-token";
    mockedUpdate.mockRejectedValueOnce(new Error("save failed"));

    await expect(store.saveDraft()).rejects.toThrow("save failed");

    expect(store.draftToken).toBe("unsaved-token");
    expect(store.isSaving).toBe(false);
  });
});
