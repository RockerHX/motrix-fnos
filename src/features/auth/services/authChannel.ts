import type { AuthChannelMessage } from "../types";

const CHANNEL_NAME = "motrix-fnos-auth";

export interface AuthChannel {
  post: (message: AuthChannelMessage) => void;
  close: () => void;
}

export function createAuthChannel(onMessage: (message: AuthChannelMessage) => void): AuthChannel | null {
  if (typeof BroadcastChannel === "undefined") {
    return null;
  }
  const channel = new BroadcastChannel(CHANNEL_NAME);
  channel.addEventListener("message", (event: MessageEvent<AuthChannelMessage>) => {
    if (event.data?.type === "auth-invalidated" || event.data?.type === "auth-updated") {
      onMessage(event.data);
    }
  });
  return {
    post: (message) => channel.postMessage(message),
    close: () => channel.close(),
  };
}
