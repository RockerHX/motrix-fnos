import { afterEach, vi } from "vitest";

const resizeObserverStub = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

const eventSourceStub = class EventSourceStub {
  url: string;
  withCredentials = false;
  readyState = 0;

  constructor(url: string | URL) {
    this.url = String(url);
  }

  addEventListener() {}
  removeEventListener() {}
  dispatchEvent() {
    return true;
  }
  close() {
    this.readyState = 2;
  }
};

if (typeof window !== "undefined" && typeof window.matchMedia !== "function") {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      media: query,
      matches: false,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
}

if (typeof globalThis.ResizeObserver === "undefined") {
  Object.defineProperty(globalThis, "ResizeObserver", {
    configurable: true,
    writable: true,
    value: resizeObserverStub,
  });
}

if (typeof globalThis.EventSource === "undefined") {
  Object.defineProperty(globalThis, "EventSource", {
    configurable: true,
    writable: true,
    value: eventSourceStub,
  });
}

afterEach(() => {
  vi.clearAllMocks();
});
