import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  getLanJsonRpcStatus,
  rotateLanJsonRpcToken,
  updateLanJsonRpcEnabled,
} from "../services/lanJsonRpcService";
import { useLanJsonRpcStore } from "./lanJsonRpcStore";

vi.mock("../services/lanJsonRpcService", () => ({
  getLanJsonRpcStatus: vi.fn(),
  rotateLanJsonRpcToken: vi.fn(),
  updateLanJsonRpcEnabled: vi.fn(),
}));

const mockedGet = vi.mocked(getLanJsonRpcStatus);
const mockedUpdate = vi.mocked(updateLanJsonRpcEnabled);
const mockedRotate = vi.mocked(rotateLanJsonRpcToken);
const disabledStatus = { enabled: false, configured: true, maskedToken: "••••••••abcd", port: 17082 };

describe("lanJsonRpcStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("keeps the controlled switch unchanged when the update fails", async () => {
    const store = useLanJsonRpcStore();
    mockedGet.mockResolvedValueOnce(disabledStatus);
    await store.loadStatus();
    mockedUpdate.mockRejectedValueOnce(new Error("save failed"));

    await expect(store.setEnabled(true)).rejects.toThrow("save failed");

    expect(store.status).toEqual(disabledStatus);
    expect(store.isSaving).toBe(false);
    expect(store.issuedToken).toBe("");
  });

  it("retains a one-time token only until sensitive state is cleared", async () => {
    const store = useLanJsonRpcStore();
    mockedUpdate.mockResolvedValueOnce({
      status: { enabled: true, configured: true, maskedToken: "••••••••1234", port: 17082 },
      issuedToken: "first-lan-token",
    });

    await store.setEnabled(true);
    expect(store.issuedToken).toBe("first-lan-token");
    store.clearSensitiveState();
    expect(store.issuedToken).toBe("");

    mockedRotate.mockResolvedValueOnce({
      status: { enabled: true, configured: true, maskedToken: "••••••••5678", port: 17082 },
      issuedToken: "rotated-lan-token",
    });
    await store.rotateToken();
    expect(store.issuedToken).toBe("rotated-lan-token");
    store.clearIssuedToken();
    expect(store.issuedToken).toBe("");
  });

  it("does not restore a one-time Token when a request completes after settings closes", async () => {
    const store = useLanJsonRpcStore();
    let resolveUpdate!: (value: Awaited<ReturnType<typeof updateLanJsonRpcEnabled>>) => void;
    mockedUpdate.mockImplementationOnce(
      () => new Promise((resolve) => {
        resolveUpdate = resolve;
      }),
    );

    const pending = store.setEnabled(true);
    store.clearSensitiveState();
    resolveUpdate({
      status: { enabled: true, configured: true, maskedToken: "••••••••late", port: 17082 },
      issuedToken: "late-secret",
    });
    await pending;

    expect(store.status?.enabled).toBe(true);
    expect(store.issuedToken).toBe("");
  });
});
