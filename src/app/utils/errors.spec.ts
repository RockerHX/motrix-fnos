import { describe, expect, it } from "vitest";
import { getErrorMessage } from "./errors";

describe("getErrorMessage", () => {
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
