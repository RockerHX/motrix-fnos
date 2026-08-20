import { ref } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DebugLogEntry } from "../types";
import { useDebugLogExport } from "./useDebugLogExport";

const message = vi.hoisted(() => ({ success: vi.fn(), warning: vi.fn() }));

vi.mock("naive-ui", () => ({ useMessage: () => message }));

const logs: DebugLogEntry[] = [
  {
    id: 1,
    timestampMs: 1_700_000_000_000,
    lastTimestampMs: 1_700_000_060_000,
    level: "warn",
    category: "aria2",
    module: "aria2.rpc",
    message: "retry",
    repeatCount: 3,
  },
];

describe("useDebugLogExport", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.defineProperty(document, "execCommand", {
      configurable: true,
      value: vi.fn(() => false),
    });
  });

  it("formats repeated log entries and falls back to manual copy", async () => {
    const onManualCopy = vi.fn();
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: vi.fn().mockRejectedValue(new Error("denied")) },
    });
    const exporter = createExporter(onManualCopy);

    await exporter.copyAllLogs();

    expect(onManualCopy).toHaveBeenCalledWith(expect.stringContaining("[WARN] [Aria2] [aria2.rpc] x3"));
    expect(onManualCopy).toHaveBeenCalledWith(expect.stringContaining("Total: 1; Filtered: 1; Warnings: 1; Errors: 0"));
    expect(message.warning).toHaveBeenCalledWith(
      "当前页面不是可使用剪贴板的安全顶层环境，常见原因是局域网 HTTP 或 fnOS 内嵌窗口。请手动选择内容并按 Ctrl+C / Command+C，或直接打开 Motrix HTTPS 域名。",
    );
  });

  it("warns when no filtered logs can be copied or downloaded", async () => {
    const exporter = useDebugLogExport({
      logs: ref(logs),
      filteredLogs: ref([]),
      warningCount: ref(1),
      errorCount: ref(0),
      onManualCopy: vi.fn(),
    });

    await exporter.copyAllLogs();
    exporter.downloadAllLogs();

    expect(message.warning).toHaveBeenCalledWith("当前没有可复制的调试日志");
    expect(message.warning).toHaveBeenCalledWith("当前没有可下载的调试日志");
  });

  it("downloads formatted logs and releases the object URL", () => {
    const createObjectURL = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:debug-log");
    const revokeObjectURL = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => undefined);
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    const exporter = createExporter(vi.fn());

    exporter.downloadAllLogs();

    expect(createObjectURL).toHaveBeenCalledOnce();
    expect(click).toHaveBeenCalledOnce();
    expect(revokeObjectURL).toHaveBeenCalledWith("blob:debug-log");
    expect(message.success).toHaveBeenCalledWith("调试日志已导出");
  });
});

function createExporter(onManualCopy: (text: string) => void) {
  return useDebugLogExport({
    logs: ref(logs),
    filteredLogs: ref(logs),
    warningCount: ref(1),
    errorCount: ref(0),
    onManualCopy,
  });
}
