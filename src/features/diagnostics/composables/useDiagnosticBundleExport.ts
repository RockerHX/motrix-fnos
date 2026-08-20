import { useMessage } from "naive-ui";
import { ref } from "vue";
import { getErrorMessage } from "../../../app/utils/errors";
import { useI18n } from "../../../i18n";
import { downloadDiagnosticBundle } from "../services/diagnosticBundleService";

const DIAGNOSTIC_BUNDLE_FILE_NAME = "motrix-fnos-diagnostic-bundle.zip";

export function useDiagnosticBundleExport() {
  const message = useMessage();
  const { t } = useI18n();
  const isExporting = ref(false);

  async function exportDiagnosticBundle() {
    if (isExporting.value) {
      return;
    }

    isExporting.value = true;
    try {
      downloadBlob(await downloadDiagnosticBundle());
      message.success(t("diagnostics.bundle.exported"));
    } catch (error) {
      message.error(getErrorMessage(error, t("diagnostics.bundle.exportFailed")));
    } finally {
      isExporting.value = false;
    }
  }

  return { isExporting, exportDiagnosticBundle };
}

function downloadBlob(blob: Blob) {
  const url = URL.createObjectURL(blob);
  try {
    const link = document.createElement("a");
    link.href = url;
    link.download = DIAGNOSTIC_BUNDLE_FILE_NAME;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  } finally {
    URL.revokeObjectURL(url);
  }
}
