import { computed, type Ref } from "vue";
import { useI18n } from "../../../i18n";
import type { TaskCreateFormState, TaskCreateInputType } from "./taskCreateFormModel";

export function useTaskCreateValidation(form: TaskCreateFormState, activeInputType: Ref<TaskCreateInputType>) {
  const { t } = useI18n();
  const isMagnetValid = computed(() => /^magnet:\?/i.test(form.magnet.trim()));
  const urlList = computed(() => form.urls.split(/\r?\n/).map((url) => url.trim()).filter(Boolean));
  const invalidUrlLines = computed(() =>
    form.urls
      .split(/\r?\n/)
      .map((url, index) => ({ url: url.trim(), line: index + 1 }))
      .filter(({ url }) => url && !/^https?:\/\/.+/i.test(url))
      .map(({ line }) => line),
  );
  const urlFeedback = computed(() => {
    if (invalidUrlLines.value.length > 0) {
      return t("create.url.invalidLines", { lines: invalidUrlLines.value.join(", ") });
    }
    if (urlList.value.length > 0) {
      return t("create.url.detected", { count: urlList.value.length });
    }
    return t("create.url.hint");
  });
  const urlValidationStatus = computed(() => (invalidUrlLines.value.length > 0 ? "error" : undefined));
  const magnetFeedback = computed(() =>
    form.magnet && !isMagnetValid.value ? t("create.magnet.invalid") : undefined,
  );
  const magnetValidationStatus = computed(() => (form.magnet && !isMagnetValid.value ? "error" : undefined));
  const hasValidAdvancedOptions = computed(
    () => form.connections >= 1 && form.connections <= 64 && form.downloadLimitKb >= 0,
  );
  const hasValidSourceInput = computed(() => {
    if (activeInputType.value === "url") {
      return urlList.value.length > 0 && invalidUrlLines.value.length === 0;
    }
    if (activeInputType.value === "torrent") {
      return !!form.torrentFile;
    }
    return isMagnetValid.value;
  });

  function validationError() {
    if (activeInputType.value === "url" && !hasValidSourceInput.value) return t("create.url.required");
    if (activeInputType.value === "torrent" && !form.torrentFile) return t("create.torrent.required");
    if (activeInputType.value === "magnet" && !isMagnetValid.value) return t("create.magnet.required");
    if (!form.saveDir) return t("create.saveDir.required");
    if (!hasValidAdvancedOptions.value) return t("create.advanced.invalid");
    return null;
  }

  return {
    isMagnetValid,
    urlList,
    invalidUrlLines,
    urlFeedback,
    urlValidationStatus,
    magnetFeedback,
    magnetValidationStatus,
    hasValidAdvancedOptions,
    hasValidSourceInput,
    validationError,
  };
}
