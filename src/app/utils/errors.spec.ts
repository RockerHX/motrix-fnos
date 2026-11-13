import { describe, expect, it } from "vitest";
import { getErrorMessage } from "./errors";
import { ApiError } from "../../services/http";
import { setLanguage } from "../../i18n";

describe("getErrorMessage", () => {
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
