import { beforeEach, describe, expect, it, vi } from "vitest";
import { language, saveLocalLanguagePreference, setLanguage } from "../i18n";
import type { FnosHostKind, FnosHostSubscription, FnosPlatformConfig, FnosTheme } from "../services/fnos";
import { FnosPlatformController } from "./hostPlatform";
import { appTheme, setAppTheme } from "./theme";

function host(kind: FnosHostKind, config: FnosPlatformConfig | null = platformConfig()) {
  let themeListener: ((theme: FnosTheme) => void) | undefined;
  let languageListener: ((language: string) => void) | undefined;
  const themeUnsubscribe = vi.fn();
  const languageUnsubscribe = vi.fn();
  return {
    adapter: {
      getHostKind: vi.fn().mockResolvedValue(kind),
      getPlatformConfig: vi.fn().mockResolvedValue(config),
      setTitle: vi.fn().mockResolvedValue({ status: "opened" }),
      subscribeTheme: vi.fn(async (listener: (theme: FnosTheme) => void) => {
        themeListener = listener;
        return subscription(themeUnsubscribe);
      }),
      subscribeLanguage: vi.fn(async (listener: (language: string) => void) => {
        languageListener = listener;
        return subscription(languageUnsubscribe);
      }),
    },
    emitTheme: (theme: FnosTheme) => themeListener?.(theme),
    emitLanguage: (nextLanguage: string) => languageListener?.(nextLanguage),
    themeUnsubscribe,
    languageUnsubscribe,
  };
}

function platformConfig(): FnosPlatformConfig {
  return {
    theme: "light",
    language: "en-US",
    systemVersion: "1.2.0401",
    format: {},
  };
}

function subscription(unsubscribe: () => void): FnosHostSubscription {
  return { status: "subscribed", unsubscribe };
}

describe("FnosPlatformController", () => {
  beforeEach(() => {
    localStorage.clear();
    setLanguage("zh-CN");
    setAppTheme("dark");
  });

  it("initializes and listens to desktop theme and language", async () => {
    const hosted = host("hosted");
    const controller = new FnosPlatformController(hosted.adapter, () => true);

    await controller.initialize();
    expect(appTheme.value).toBe("light");
    expect(language.value).toBe("en-US");
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(hosted.adapter.setTitle).toHaveBeenCalledWith("Motrix");

    hosted.emitTheme("dark");
    hosted.emitLanguage("zh-CN");
    expect(appTheme.value).toBe("dark");
    expect(language.value).toBe("zh-CN");

    controller.dispose();
    expect(hosted.themeUnsubscribe).toHaveBeenCalledOnce();
    expect(hosted.languageUnsubscribe).toHaveBeenCalledOnce();
  });

  it("initializes mobile theme without subscribing to Web events", async () => {
    const mobile = host("mobile");
    const controller = new FnosPlatformController(mobile.adapter, () => true);

    await controller.initialize();

    expect(appTheme.value).toBe("light");
    expect(mobile.adapter.subscribeTheme).not.toHaveBeenCalled();
    expect(mobile.adapter.subscribeLanguage).not.toHaveBeenCalled();
  });

  it("leaves standalone browsers on the default dark theme without host calls", async () => {
    const standalone = host("standalone");
    const controller = new FnosPlatformController(standalone.adapter, () => true);

    await controller.initialize();

    expect(appTheme.value).toBe("dark");
    expect(standalone.adapter.getPlatformConfig).not.toHaveBeenCalled();
    expect(standalone.adapter.setTitle).not.toHaveBeenCalled();
  });

  it("never overrides explicit local or post-login language choices", async () => {
    saveLocalLanguagePreference("zh-CN");
    const localPreference = host("hosted");
    await new FnosPlatformController(localPreference.adapter, () => true).initialize();
    expect(language.value).toBe("zh-CN");

    localStorage.clear();
    const postLogin = host("hosted");
    const controller = new FnosPlatformController(postLogin.adapter, () => false);
    await controller.initialize();
    postLogin.emitLanguage("en-US");
    expect(language.value).toBe("zh-CN");
    controller.dispose();
  });
});
