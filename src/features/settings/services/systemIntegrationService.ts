import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";

export async function getAutoStartEnabled() {
  return isEnabled();
}

export async function setAutoStartEnabled(enabled: boolean) {
  if (enabled) {
    await enable();
    return;
  }

  await disable();
}
