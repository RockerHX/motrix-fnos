import { beforeEach, describe, expect, it, vi } from "vitest";
import { useDiagnosticBundleExport } from "./useDiagnosticBundleExport";

const message = vi.hoisted(() => ({ success: vi.fn(), error: vi.fn() }));
const diagnosticBundleService = vi.hoisted(() => ({ downloadDiagnosticBundle: vi.fn() }));

vi.mock("naive-ui", () => ({ useMessage: () => message }));
vi.mock("../services/diagnosticBundleService", () => diagnosticBundleService);

describe("useDiagnosticBundleExport", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("downloads the generated bundle and releases its object URL", async () => {
    diagnosticBundleService.downloadDiagnosticBundle.mockResolvedValue(new Blob(["zip"], { type: "application/zip" }));
    const createObjectURL = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:diagnostic-bundle");
    const revokeObjectURL = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => undefined);
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    const exporter = useDiagnosticBundleExport();

    await exporter.exportDiagnosticBundle();

    expect(createObjectURL).toHaveBeenCalledOnce();
    expect(click).toHaveBeenCalledOnce();
    expect(revokeObjectURL).toHaveBeenCalledWith("blob:diagnostic-bundle");
    expect(message.success).toHaveBeenCalledWith("诊断包已导出");
    expect(exporter.isExporting.value).toBe(false);
  });

  it("shows an error when the bundle request fails", async () => {
    diagnosticBundleService.downloadDiagnosticBundle.mockRejectedValue(new Error("network unavailable"));
    const exporter = useDiagnosticBundleExport();

    await exporter.exportDiagnosticBundle();

    expect(message.error).toHaveBeenCalledWith("network unavailable");
    expect(exporter.isExporting.value).toBe(false);
  });
});
