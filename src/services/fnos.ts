import type { AppBridgeResponse, PlatformConfig } from "@trimjs/web-app";

export type FnosHostKind = "hosted" | "mobile" | "standalone" | "unavailable";

export type SharedFolderAuthorizationResult =
  | { status: "authorized" }
  | { status: "cancelled" }
  | { status: "admin_required" }
  | { status: "unsupported" }
  | { status: "failed" };

export type FnosHostActionResult = { status: "opened" | "unsupported" | "failed" };
export type FnosTheme = "dark" | "light";
export type FnosPlatformConfig = PlatformConfig;
export type FnosHostSubscription = {
  status: "subscribed" | "unsupported" | "failed";
  unsubscribe: () => void;
};

type TrimAppLike = {
  isWeb: boolean;
  isStandaloneWeb: boolean;
  ready(): Promise<void>;
  pickSharedFile(): Promise<AppBridgeResponse<string[]> | undefined>;
  openAppSetting(): Promise<unknown>;
  getPlatformConfig(): Promise<PlatformConfig>;
  setTitle(title: string): Promise<unknown>;
  openFile(path: string): Promise<unknown>;
  openFileManager(path: string): Promise<unknown>;
  showFileDetails(paths: string[]): Promise<unknown>;
  $on(event: string, callback: (...args: unknown[]) => void): Promise<void>;
  $off(event: string, callback: (...args: unknown[]) => void): Promise<void>;
};

type TrimAppModule = {
  TrimApp: new () => TrimAppLike;
};

type TrimAppLoader = () => Promise<TrimAppModule>;

type FnosRuntime = {
  kind: FnosHostKind;
  app?: TrimAppLike;
};

const ADMIN_REQUIRED_CODES = new Set([1, 1_000_002, 1_003_201]);
const UNSUPPORTED_CODES = new Set([1_000_030, 1_000_300, 1_003_103]);

export class FnosHostAdapter {
  private runtimePromise: Promise<FnosRuntime> | null = null;

  constructor(private readonly loadTrimApp: TrimAppLoader = loadTrimAppModule) {}

  async getHostKind(): Promise<FnosHostKind> {
    return (await this.runtime()).kind;
  }

  async requestSharedFolderAuthorization(): Promise<SharedFolderAuthorizationResult> {
    const runtime = await this.runtime();
    if (!runtime.app || runtime.kind === "standalone" || runtime.kind === "unavailable") {
      return { status: "unsupported" };
    }

    try {
      const result = await runtime.app.pickSharedFile();
      if (result === undefined) return { status: "cancelled" };
      if (result.code === 0) return { status: "authorized" };
      if (ADMIN_REQUIRED_CODES.has(result.code) || isAdministratorMessage(result.msg)) {
        return { status: "admin_required" };
      }
      if (UNSUPPORTED_CODES.has(result.code)) return { status: "unsupported" };
      return { status: "failed" };
    } catch (error) {
      const message = errorMessage(error);
      if (message === "Operation failed") return { status: "cancelled" };
      if (isAdministratorMessage(message)) return { status: "admin_required" };
      if (isUnsupportedMessage(message)) return { status: "unsupported" };
      return { status: "failed" };
    }
  }

  async openAppSettings(): Promise<FnosHostActionResult> {
    const runtime = await this.runtime();
    if (!runtime.app || runtime.kind === "standalone" || runtime.kind === "unavailable") {
      return { status: "unsupported" };
    }
    try {
      await runtime.app.openAppSetting();
      return { status: "opened" };
    } catch {
      return { status: "failed" };
    }
  }

  async getPlatformConfig(): Promise<FnosPlatformConfig | null> {
    const runtime = await this.supportedRuntime();
    if (!runtime) return null;
    try {
      return await runtime.app.getPlatformConfig();
    } catch {
      return null;
    }
  }

  async setTitle(title: string): Promise<FnosHostActionResult> {
    return this.runHostAction((app) => app.setTitle(title));
  }

  async openFile(path: string): Promise<FnosHostActionResult> {
    if (!path.trim()) return { status: "failed" };
    return this.runHostAction((app) => app.openFile(path));
  }

  async openFileManager(path: string): Promise<FnosHostActionResult> {
    if (!path.trim()) return { status: "failed" };
    return this.runHostAction((app) => app.openFileManager(path));
  }

  async showFileDetails(paths: string[]): Promise<FnosHostActionResult> {
    if (paths.length === 0 || paths.some((path) => !path.trim())) return { status: "failed" };
    return this.runHostAction((app) => app.showFileDetails(paths));
  }

  async subscribeTheme(listener: (theme: FnosTheme) => void): Promise<FnosHostSubscription> {
    return this.subscribeHostedEvent("os/theme", (value) => {
      if (value === "dark" || value === "light") listener(value);
    });
  }

  async subscribeLanguage(listener: (language: string) => void): Promise<FnosHostSubscription> {
    return this.subscribeHostedEvent("os/language", (value) => {
      if (typeof value === "string" && value.trim()) listener(value);
    });
  }

  private async runHostAction(action: (app: TrimAppLike) => Promise<unknown>): Promise<FnosHostActionResult> {
    const runtime = await this.supportedRuntime();
    if (!runtime) return { status: "unsupported" };
    try {
      await action(runtime.app);
      return { status: "opened" };
    } catch {
      return { status: "failed" };
    }
  }

  private async subscribeHostedEvent(
    event: string,
    listener: (...args: unknown[]) => void,
  ): Promise<FnosHostSubscription> {
    const runtime = await this.runtime();
    if (!runtime.app || runtime.kind !== "hosted") return unsupportedSubscription();
    try {
      await runtime.app.$on(event, listener);
      let active = true;
      return {
        status: "subscribed",
        unsubscribe: () => {
          if (!active) return;
          active = false;
          void runtime.app?.$off(event, listener).catch(() => undefined);
        },
      };
    } catch {
      return failedSubscription();
    }
  }

  private async supportedRuntime(): Promise<(FnosRuntime & { app: TrimAppLike }) | null> {
    const runtime = await this.runtime();
    if (!runtime.app || runtime.kind === "standalone" || runtime.kind === "unavailable") {
      return null;
    }
    return runtime as FnosRuntime & { app: TrimAppLike };
  }

  private runtime(): Promise<FnosRuntime> {
    if (!this.runtimePromise) this.runtimePromise = this.initialize();
    return this.runtimePromise;
  }

  private async initialize(): Promise<FnosRuntime> {
    try {
      const module = await this.loadTrimApp();
      const app = new module.TrimApp();
      await app.ready();
      if (app.isStandaloneWeb) return { kind: "standalone", app };
      return { kind: app.isWeb ? "hosted" : "mobile", app };
    } catch {
      return { kind: "unavailable" };
    }
  }
}

export const fnosHost = new FnosHostAdapter();

function unsupportedSubscription(): FnosHostSubscription {
  return { status: "unsupported", unsubscribe: () => undefined };
}

function failedSubscription(): FnosHostSubscription {
  return { status: "failed", unsubscribe: () => undefined };
}

async function loadTrimAppModule(): Promise<TrimAppModule> {
  return import("@trimjs/web-app");
}

function errorMessage(error: unknown) {
  if (error instanceof Error) return error.message.trim();
  return String(error).trim();
}

function isAdministratorMessage(message: string) {
  const normalized = message.toLowerCase();
  return normalized.includes("仅管理员") || normalized.includes("administrator") || normalized.includes("admin permission");
}

function isUnsupportedMessage(message: string) {
  const normalized = message.toLowerCase();
  return normalized.includes("not supported") || normalized.includes("unsupported") || normalized.includes("app runtime");
}
