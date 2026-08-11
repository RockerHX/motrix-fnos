import { httpGetBlob } from "../../../services/http";

export function downloadDiagnosticBundle(): Promise<Blob> {
  return httpGetBlob("/api/diagnostics/diagnostic-bundle");
}
