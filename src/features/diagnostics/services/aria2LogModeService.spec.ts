import { beforeEach, describe, expect, it, vi } from "vitest";
import { httpGet, httpPut } from "../../../services/http";
import { getAria2LogMode, updateAria2LogMode } from "./aria2LogModeService";

vi.mock("../../../services/http", () => ({ httpGet: vi.fn(), httpPut: vi.fn() }));

describe("aria2LogModeService", () => {
  beforeEach(() => vi.clearAllMocks());

  it("uses the authenticated diagnostics endpoints", () => {
    getAria2LogMode();
    updateAria2LogMode(true);
    updateAria2LogMode(false);

    expect(httpGet).toHaveBeenCalledWith("/api/diagnostics/aria2-log-mode");
    expect(httpPut).toHaveBeenNthCalledWith(1, "/api/diagnostics/aria2-log-mode", { detailed: true });
    expect(httpPut).toHaveBeenNthCalledWith(2, "/api/diagnostics/aria2-log-mode", { detailed: false });
  });
});
