import { normalizeLanguage, setLanguage, hasLocalLanguagePreference } from "../i18n";
import {
  fnosHost,
  type FnosHostAdapter,
  type FnosHostSubscription,
} from "../services/fnos";
import { setAppTheme } from "./theme";

type HostAdapter = Pick<
  FnosHostAdapter,
  "getHostKind" | "getPlatformConfig" | "setTitle" | "subscribeTheme" | "subscribeLanguage"
>;

export class FnosPlatformController {
  private subscriptions: FnosHostSubscription[] = [];
  private active = false;

  constructor(
    private readonly host: HostAdapter = fnosHost,
    private readonly canUseHostLanguage: () => boolean = () => true,
  ) {}

  async initialize() {
    this.dispose();
    this.active = true;
    const kind = await this.host.getHostKind();
    if (!this.active || kind === "standalone" || kind === "unavailable") return;

    const config = await this.host.getPlatformConfig();
    if (!this.active) return;
    if (config) {
      setAppTheme(config.theme);
      this.applyHostLanguage(config.language);
    }
    void this.host.setTitle("Motrix");

    if (kind !== "hosted") return;
    const [themeSubscription, languageSubscription] = await Promise.all([
      this.host.subscribeTheme((theme) => {
        if (this.active) setAppTheme(theme);
      }),
      this.host.subscribeLanguage((language) => {
        if (this.active) this.applyHostLanguage(language);
      }),
    ]);
    if (!this.active) {
      themeSubscription.unsubscribe();
      languageSubscription.unsubscribe();
      return;
    }
    this.subscriptions.push(themeSubscription, languageSubscription);
  }

  dispose() {
    this.active = false;
    for (const subscription of this.subscriptions.splice(0)) subscription.unsubscribe();
  }

  private applyHostLanguage(language: string) {
    if (!this.canUseHostLanguage() || hasLocalLanguagePreference()) return;
    setLanguage(normalizeLanguage(language));
  }
}

export function createFnosPlatformController(canUseHostLanguage: () => boolean) {
  return new FnosPlatformController(fnosHost, canUseHostLanguage);
}
