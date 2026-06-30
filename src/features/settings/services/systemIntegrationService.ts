import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { isPermissionGranted, requestPermission } from "@tauri-apps/plugin-notification";

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

export async function ensureNotificationPermission() {
  if (await isPermissionGranted()) {
    return true;
  }

  const permission = await requestPermission();
  return permission === "granted";
}
