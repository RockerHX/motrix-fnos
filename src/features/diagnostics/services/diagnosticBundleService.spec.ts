import { beforeEach, describe, expect, it, vi } from "vitest";
import { httpGetBlob } from "../../../services/http";
import { downloadDiagnosticBundle } from "./diagnosticBundleService";

vi.mock("../../../services/http", () => ({ httpGetBlob: vi.fn() }));

describe("diagnostic bundle service", () => {
  beforeEach(() => vi.clearAllMocks());

  it("requests the protected diagnostic bundle endpoint", () => {
    downloadDiagnosticBundle();

    expect(httpGetBlob).toHaveBeenCalledWith("/api/diagnostics/diagnostic-bundle");
  });
});
