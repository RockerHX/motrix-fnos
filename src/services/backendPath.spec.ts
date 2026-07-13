import { afterEach, describe, expect, it } from "vitest";
import { backendPath } from "./backendPath";

describe("backendPath", () => {
  afterEach(() => {
    window.history.replaceState({}, "", "/");
  });

  it("keeps local development api paths unchanged", () => {
    expect(backendPath("/api/app/ping")).toBe("/api/app/ping");
  });

  it("adds the fnOS gateway prefix when opened through the gateway", () => {
    window.history.replaceState({}, "", "/app/motrix/");

    expect(backendPath("/api/app/ping")).toBe("/app/motrix/api/app/ping");
  });
});
