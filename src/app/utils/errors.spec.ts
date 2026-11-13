import { describe, expect, it } from "vitest";
import { getErrorMessage } from "./errors";
import { ApiError } from "../../services/http";
import { setLanguage } from "../../i18n";

describe("getErrorMessage", () => {
  it.each([
    ["runtime_transition", 409, "Aria2 正在切换运行状态，请稍后重试", /will be retried|next runs/i],
    ["proxy_apply_failed", 502, "更新任务选项失败：请求被拒绝", /will be retried|next runs/i],
    ["aria2_log_mode_update_failed", 503, "修改 Aria2 日志级别失败：请求被拒绝", /saved|enabled/i],
  ])("does not claim an unapplied change was saved or scheduled for %s", (code, status, original, unsupportedClaim) => {
    const error = new ApiError(status, { code, message: original });
    try {
      setLanguage("en-US");
      const displayed = getErrorMessage(error, "fallback");
      expect(displayed).not.toMatch(unsupportedClaim);
      expect(displayed).not.toMatch(/\p{Script=Han}/u);
      expect(displayed).toMatch(/retry|current/i);
      expect(error.message).toBe(original);
      setLanguage("zh-CN");
      expect(getErrorMessage(error, "fallback")).toBe(original);
    } finally {
      setLanguage("zh-CN");
    }
  });

  it("translates API errors for display while retaining their original diagnostic data", () => {
    const error = new ApiError(401, { code: "invalid_credentials", message: "管理密码无效" });
    try {
      setLanguage("en-US");
      expect(getErrorMessage(error, "fallback")).toBe("The management password is incorrect.");
      expect(error.message).toBe("管理密码无效");
      expect(error.code).toBe("invalid_credentials");
      setLanguage("zh-CN");
      expect(getErrorMessage(error, "fallback")).toBe("管理密码无效");
    } finally {
      setLanguage("zh-CN");
    }
  });

  it("returns Error message", () => {
    expect(getErrorMessage(new Error("boom"), "fallback")).toBe("boom");
  });

  it("returns string errors", () => {
    expect(getErrorMessage("failed", "fallback")).toBe("failed");
  });

  it("returns number errors as strings", () => {
    expect(getErrorMessage(404, "fallback")).toBe("404");
  });

  it("returns fallback for empty messages", () => {
    expect(getErrorMessage("", "fallback")).toBe("fallback");
  });
});
