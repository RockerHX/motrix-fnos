import { afterEach, describe, expect, it, vi } from "vitest";
import { createAuthChannel } from "./authChannel";

describe("authChannel", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("broadcasts only non-sensitive auth event types", () => {
    const posted: unknown[] = [];
    const listeners: Array<(event: MessageEvent) => void> = [];
    const close = vi.fn();
    class ChannelMock {
      addEventListener(_type: string, next: (event: MessageEvent) => void) {
        listeners.push(next);
      }
      postMessage(message: unknown) {
        posted.push(message);
      }
      close = close;
    }
    vi.stubGlobal("BroadcastChannel", ChannelMock);
    const onMessage = vi.fn();
    const channel = createAuthChannel(onMessage);

    channel?.post({ type: "session-invalidated" });
    channel?.post({ type: "auth-updated" });
    expect(posted).toEqual([{ type: "session-invalidated" }, { type: "auth-updated" }]);
    expect(JSON.stringify(posted)).not.toMatch(/csrf|password|cookie|sessionId/i);

    listeners[0]?.({ data: { type: "session-invalidated" } } as MessageEvent);
    expect(onMessage).toHaveBeenCalledWith({ type: "session-invalidated" });
    channel?.close();
    expect(close).toHaveBeenCalledOnce();
  });
});
