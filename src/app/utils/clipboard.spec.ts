import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { copyTextToClipboard } from "./clipboard";

describe("copyTextToClipboard", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    document.body.innerHTML = "";
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: undefined });
    Object.defineProperty(window, "isSecureContext", { configurable: true, value: true });
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("uses the modern Clipboard API when available", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    const execCommand = installExecCommand(true);
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });

    await expect(copyTextToClipboard("modern-copy")).resolves.toEqual({
      copied: true,
      method: "clipboard",
    });
    expect(writeText).toHaveBeenCalledWith("modern-copy");
    expect(execCommand).not.toHaveBeenCalled();
  });

  it("uses the legacy command synchronously when the Clipboard API is unavailable", async () => {
    const execCommand = installExecCommand(true);
    const resultPromise = copyTextToClipboard("legacy-copy");

    expect(execCommand).toHaveBeenCalledWith("copy");
    await expect(resultPromise).resolves.toEqual({ copied: true, method: "legacy" });
    expect(document.querySelector("textarea")).toBeNull();
  });

  it("falls back to the legacy command when Clipboard API permission is denied", async () => {
    const writeText = vi.fn().mockRejectedValue(new DOMException("denied", "NotAllowedError"));
    const execCommand = installExecCommand(true);
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });

    await expect(copyTextToClipboard("fallback-copy")).resolves.toEqual({
      copied: true,
      method: "legacy",
    });
    expect(execCommand).toHaveBeenCalledWith("copy");
    expect(document.querySelector("textarea")).toBeNull();
  });

  it("reports an insecure context when every copy method fails", async () => {
    installExecCommand(false);
    Object.defineProperty(window, "isSecureContext", { configurable: true, value: false });

    await expect(copyTextToClipboard("manual-copy")).resolves.toEqual({
      copied: false,
      method: null,
      reason: "insecure-context",
    });
    expect(document.querySelector("textarea")).toBeNull();
  });

  it("cleans up the temporary textarea when execCommand throws", async () => {
    Object.defineProperty(document, "execCommand", {
      configurable: true,
      value: vi.fn(() => {
        throw new Error("copy failed");
      }),
    });

    await expect(copyTextToClipboard("sensitive-token")).resolves.toEqual({
      copied: false,
      method: null,
      reason: "unavailable",
    });
    expect(document.querySelector("textarea")).toBeNull();
  });

  it("does not write copied content to the console", async () => {
    installExecCommand(false);
    const log = vi.spyOn(console, "log").mockImplementation(() => undefined);
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);

    await copyTextToClipboard("never-log-this-token");

    expect(log).not.toHaveBeenCalled();
    expect(warn).not.toHaveBeenCalled();
    expect(error).not.toHaveBeenCalled();
  });
});

function installExecCommand(result: boolean) {
  const execCommand = vi.fn(() => result);
  Object.defineProperty(document, "execCommand", {
    configurable: true,
    value: execCommand,
  });
  return execCommand;
}
