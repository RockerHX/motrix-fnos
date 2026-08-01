export type ClipboardCopyMethod = "clipboard" | "legacy";
export type ClipboardFailureReason =
  | "insecure-context"
  | "embedded-context"
  | "denied"
  | "unavailable";

export type ClipboardCopyResult =
  | { copied: true; method: ClipboardCopyMethod }
  | { copied: false; method: null; reason: ClipboardFailureReason };

export async function copyTextToClipboard(text: string): Promise<ClipboardCopyResult> {
  const writeText =
    typeof navigator !== "undefined" && navigator.clipboard?.writeText
      ? navigator.clipboard.writeText.bind(navigator.clipboard)
      : null;

  if (writeText) {
    try {
      await writeText(text);
      return { copied: true, method: "clipboard" };
    } catch (error) {
      if (copyWithLegacyCommand(text)) {
        return { copied: true, method: "legacy" };
      }
      return { copied: false, method: null, reason: clipboardFailureReason(error) };
    }
  }

  if (copyWithLegacyCommand(text)) {
    return { copied: true, method: "legacy" };
  }
  return { copied: false, method: null, reason: clipboardFailureReason() };
}

function copyWithLegacyCommand(text: string): boolean {
  if (typeof document === "undefined" || typeof document.execCommand !== "function") {
    return false;
  }

  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.setAttribute("aria-hidden", "true");
  textarea.style.position = "fixed";
  textarea.style.inset = "0 auto auto -9999px";
  textarea.style.opacity = "0";
  textarea.style.pointerEvents = "none";
  document.body.appendChild(textarea);

  try {
    textarea.focus({ preventScroll: true });
    textarea.select();
    textarea.setSelectionRange(0, textarea.value.length);
    return document.execCommand("copy");
  } catch {
    return false;
  } finally {
    textarea.remove();
  }
}

function clipboardFailureReason(error?: unknown): ClipboardFailureReason {
  if (isEmbeddedWindow()) {
    return "embedded-context";
  }
  if (typeof window !== "undefined" && window.isSecureContext === false) {
    return "insecure-context";
  }
  if (error instanceof DOMException && ["NotAllowedError", "SecurityError"].includes(error.name)) {
    return "denied";
  }
  return "unavailable";
}

function isEmbeddedWindow(): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  try {
    return window.self !== window.top;
  } catch {
    return true;
  }
}
