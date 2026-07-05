import { ref } from "vue";
import { checkAppUpdate } from "../services/aboutService";
import type { AppUpdateCheck } from "../../../types/app";

interface UpdateCheckMessageApi {
  error: (content: string) => unknown;
}

interface UseUpdateCheckOptions {
  message: UpdateCheckMessageApi;
  fallbackMessage: string;
}

export function useUpdateCheck({ message, fallbackMessage }: UseUpdateCheckOptions) {
  const updateCheck = ref<AppUpdateCheck | null>(null);
  const isCheckingUpdate = ref(false);

  async function runUpdateCheck() {
    isCheckingUpdate.value = true;
    try {
      updateCheck.value = await checkAppUpdate();
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      message.error(errorMessage || fallbackMessage);
    } finally {
      isCheckingUpdate.value = false;
    }
  }

  return {
    updateCheck,
    isCheckingUpdate,
    runUpdateCheck,
  };
}
