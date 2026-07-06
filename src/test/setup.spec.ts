import { describe, expect, it } from "vitest";

describe("vitest setup", () => {
  it("provides browser compatibility stubs", () => {
    expect(window.matchMedia("(max-width: 767px)").matches).toBe(false);
    expect(typeof ResizeObserver).toBe("function");
    expect(typeof EventSource).toBe("function");
  });
});
