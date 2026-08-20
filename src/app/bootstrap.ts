export const AUTH_CONFIRMATION_MIN_VISIBLE_MS = 1000;
export const BOOTSTRAP_FADE_MS = 180;

export function createBootstrapController() {
  let dismissTimer: number | null = null;
  let removeTimer: number | null = null;
  let confirmationStartedAt: number | null = null;
  let finishing = false;

  function startConfirmation() {
    if (confirmationStartedAt !== null) return;
    confirmationStartedAt = performance.now();
    const status = document.getElementById("app-bootstrap-status");
    if (status) status.textContent = "正在确认管理访问权限…";
  }

  function finish() {
    if (finishing) return;
    const element = document.getElementById("app-bootstrap");
    if (!element) return;
    finishing = true;

    const startedAt = confirmationStartedAt ?? performance.now();
    const remaining = Math.max(0, AUTH_CONFIRMATION_MIN_VISIBLE_MS - (performance.now() - startedAt));
    dismissTimer = window.setTimeout(() => {
      element.classList.add("app-bootstrap--leaving");
      removeTimer = window.setTimeout(() => element.remove(), BOOTSTRAP_FADE_MS);
    }, remaining);
  }

  function dispose() {
    if (dismissTimer !== null) window.clearTimeout(dismissTimer);
    if (removeTimer !== null) window.clearTimeout(removeTimer);
    dismissTimer = null;
    removeTimer = null;
  }

  return { startConfirmation, finish, dispose };
}
