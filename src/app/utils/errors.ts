import { formatServerMessage } from "../../i18n/serverMessages";

export function getErrorMessage(error: unknown, fallback: string): string {
  const message = error instanceof Error ? error.message : String(error);
  const code = error instanceof Error && "code" in error && typeof error.code === "string" ? error.code : undefined;
  return formatServerMessage(message, code) || fallback;
}
