import { beforeEach, describe, expect, it, vi } from "vitest";
import { httpGet } from "../../../services/http";
import { getStorageUsage } from "./storageDiagnosticsService";

vi.mock("../../../services/http", () => ({ httpGet: vi.fn() }));

describe("storage diagnostics service", () => {
  beforeEach(() => vi.clearAllMocks());

  it("uses the protected storage usage endpoint", () => {
    getStorageUsage();

    expect(httpGet).toHaveBeenCalledWith("/api/diagnostics/storage");
  });
});
