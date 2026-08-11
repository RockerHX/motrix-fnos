import { beforeEach, describe, expect, it, vi } from "vitest";
import { httpDelete, httpGet } from "../../../services/http";
import { clearAria2Logs, getLogUsage } from "./logMaintenanceService";

vi.mock("../../../services/http", () => ({ httpDelete: vi.fn(), httpGet: vi.fn() }));

describe("log maintenance service", () => {
  beforeEach(() => vi.clearAllMocks());

  it("uses the protected log usage and cleanup endpoints", () => {
    getLogUsage();
    clearAria2Logs();

    expect(httpGet).toHaveBeenCalledWith("/api/diagnostics/logs");
    expect(httpDelete).toHaveBeenCalledWith("/api/diagnostics/aria2-logs");
  });
});
