import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BOOTSTRAP_FADE_MS, createBootstrapController } from "./bootstrap";

describe("bootstrap controller", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    document.body.innerHTML = '<main id="app-bootstrap"><span id="app-bootstrap-status">正在建立安全连接…</span></main>';
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
    document.body.innerHTML = "";
  });

  it("only fills the remaining part of the one-second auth confirmation duration", () => {
    vi.spyOn(performance, "now").mockReturnValueOnce(400).mockReturnValueOnce(700);
    const controller = createBootstrapController();

    controller.startConfirmation();
    expect(document.getElementById("app-bootstrap-status")?.textContent).toBe("正在确认管理访问权限…");
    controller.finish();
    vi.advanceTimersByTime(699);
    expect(document.getElementById("app-bootstrap")?.classList.contains("app-bootstrap--leaving")).toBe(false);
    vi.advanceTimersByTime(1);
    expect(document.getElementById("app-bootstrap")?.classList.contains("app-bootstrap--leaving")).toBe(true);
    vi.advanceTimersByTime(BOOTSTRAP_FADE_MS);
    expect(document.getElementById("app-bootstrap")).toBeNull();
  });

  it("does not add a delay after auth confirmation already took longer than one second", () => {
    vi.spyOn(performance, "now").mockReturnValueOnce(100).mockReturnValueOnce(1400);
    const controller = createBootstrapController();

    controller.startConfirmation();
    controller.finish();
    vi.advanceTimersByTime(0);
    expect(document.getElementById("app-bootstrap")?.classList.contains("app-bootstrap--leaving")).toBe(true);
  });
});
