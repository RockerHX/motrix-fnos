<script setup lang="ts">
import { computed, nextTick, onMounted, reactive, ref } from "vue";
import {
  NAlert,
  NButton,
  NCard,
  NForm,
  NFormItem,
  NInput,
  NSelect,
  NSpin,
  type InputInst,
} from "naive-ui";
import { getErrorMessage } from "../../../app/utils/errors";
import {
  getLocalLanguagePreference,
  language,
  saveLocalLanguagePreference,
  setLanguage,
  supportedLanguages,
  useI18n,
  type AppLanguage,
} from "../../../i18n";
import { useAuthStore } from "../stores/authStore";
import { downloadLoginDiagnostic } from "../services/authService";

const authStore = useAuthStore();
const { t } = useI18n();
const passwordInput = ref<InputInst | null>(null);
const submitError = ref("");
const diagnosticError = ref("");
const diagnosticText = ref("");
const isDownloadingDiagnostic = ref(false);
const form = reactive({ password: "" });
const languageOptions = computed(() =>
  supportedLanguages.map((value) => ({
    value,
    label: value === "zh-CN" ? t("language.zhCN") : t("language.enUS"),
  })),
);

onMounted(() => {
  const saved = getLocalLanguagePreference();
  if (saved) setLanguage(saved);
  void focusPassword();
});

async function submit() {
  if (authStore.isSubmitting || authStore.phase !== "login") return;
  submitError.value = validateForm();
  if (submitError.value) {
    await focusPassword();
    return;
  }
  try {
    await authStore.login(form.password);
    form.password = "";
  } catch (error) {
    submitError.value = getErrorMessage(error, t("auth.submitFailed"));
    await focusPassword();
  }
}

function loginDiagnosticInfo() {
  const location = window.location;
  const origin = location.origin || `${location.protocol}//${location.host}`;
  return [
    t("auth.diagnostic.title"),
    t("auth.diagnostic.protocol", { value: location.protocol.replace(":", "") || t("common.unknown") }),
    t("auth.diagnostic.origin", { value: origin }),
    t("auth.diagnostic.iframe", { value: t(window.self !== window.top ? "common.yes" : "common.no") }),
    t("auth.diagnostic.storage", { value: t(authStore.localStorageAvailable ? "common.yes" : "common.no") }),
    t("auth.diagnostic.token", { value: t(authStore.hasAccessToken ? "common.yes" : "common.no") }),
    t("auth.diagnostic.secure", { value: t(window.isSecureContext ? "common.yes" : "common.no") }),
    t("auth.diagnostic.userAgent", { value: navigator.userAgent || t("common.unknown") }),
  ].join("\n");
}

async function copyLoginDiagnostic() {
  diagnosticError.value = "";
  const text = loginDiagnosticInfo();
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
    } else {
      const textarea = document.createElement("textarea");
      textarea.value = text;
      textarea.setAttribute("readonly", "");
      textarea.style.position = "fixed";
      textarea.style.opacity = "0";
      document.body.appendChild(textarea);
      textarea.select();
      const copied = document.execCommand("copy");
      textarea.remove();
      if (!copied) throw new Error("clipboard copy failed");
    }
  } catch {
    diagnosticText.value = text;
    diagnosticError.value = t("auth.loginDiagnosticCopyFailed");
  }
}

async function downloadDiagnostic() {
  diagnosticError.value = "";
  isDownloadingDiagnostic.value = true;
  try {
    const blob = await downloadLoginDiagnostic();
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = "motrix-fnos-login-diagnostic.zip";
    anchor.click();
    URL.revokeObjectURL(url);
  } catch {
    diagnosticError.value = t("auth.loginDiagnosticFailed");
  } finally {
    isDownloadingDiagnostic.value = false;
  }
}

function validateForm() {
  if (!form.password) return t("auth.passwordRequired");
  const charCount = Array.from(form.password).length;
  const byteCount = new TextEncoder().encode(form.password).length;
  if (charCount < 8 || charCount > 128 || byteCount > 512) return t("auth.passwordLength");
  return "";
}

function changeLanguage(value: AppLanguage) {
  setLanguage(value);
  saveLocalLanguagePreference(value);
}

async function focusPassword() {
  await nextTick();
  passwordInput.value?.focus();
}
</script>

<template>
  <main class="auth-gate">
    <NCard class="auth-card" :bordered="false">
      <header class="auth-brand">
        <img src="/icon.png" alt="" class="auth-logo" />
        <div>
          <strong>Motrix</strong>
          <p>{{ t("auth.brandSubtitle") }}</p>
        </div>
      </header>

      <div v-if="authStore.phase === 'loading'" class="auth-state" data-test="auth-loading">
        <NSpin size="large" />
        <p>{{ t("auth.loading") }}</p>
      </div>

      <div v-else-if="authStore.phase === 'error'" class="auth-state" data-test="auth-error">
        <NAlert type="error" :title="t('auth.loadFailed')">{{ authStore.errorMessage }}</NAlert>
        <NButton type="primary" :loading="authStore.isSubmitting" @click="authStore.initialize">{{ t("auth.retry") }}</NButton>
      </div>

      <div v-else-if="authStore.phase === 'unconfigured'" class="auth-state" data-test="auth-unconfigured">
        <NAlert type="warning" :title="t('auth.unconfigured.title')">{{ t("auth.unconfigured.description") }}</NAlert>
        <NButton type="primary" @click="authStore.initialize">{{ t("auth.retry") }}</NButton>
      </div>

      <NForm v-else-if="authStore.phase === 'login'" class="auth-form" :show-label="true" @submit.prevent="submit">
        <div class="auth-heading">
          <h1>{{ t("auth.login.title") }}</h1>
          <p>{{ t("auth.login.description") }}</p>
        </div>
        <NAlert v-if="submitError" type="error" data-test="auth-submit-error">{{ submitError }}</NAlert>
        <NFormItem :label="t('auth.password')">
          <NInput
            ref="passwordInput"
            v-model:value="form.password"
            type="password"
            show-password-on="mousedown"
            :placeholder="t('auth.passwordPlaceholder')"
            :input-props="{ autocomplete: 'current-password' }"
            :disabled="authStore.isSubmitting"
            data-test="auth-password"
          />
        </NFormItem>
        <NButton block type="primary" attr-type="submit" :loading="authStore.isSubmitting" data-test="auth-submit">
          {{ t("auth.login.submit") }}
        </NButton>
      </NForm>

      <section v-if="authStore.phase === 'unconfigured' || authStore.phase === 'login'" class="auth-diagnostics" data-test="auth-diagnostics">
        <p class="auth-diagnostics-title">{{ t("auth.loginDiagnosticInfo") }}</p>
        <div class="auth-diagnostics-actions">
          <NButton attr-type="button" secondary @click="copyLoginDiagnostic" data-test="auth-copy-diagnostic">
            {{ t("auth.copyLoginDiagnostic") }}
          </NButton>
          <NButton attr-type="button" secondary :loading="isDownloadingDiagnostic" @click="downloadDiagnostic" data-test="auth-download-diagnostic">
            {{ t("auth.downloadLoginDiagnostic") }}
          </NButton>
        </div>
        <NAlert v-if="diagnosticError" type="error" data-test="auth-diagnostic-error">{{ diagnosticError }}</NAlert>
        <textarea
          v-if="diagnosticText"
          class="auth-diagnostics-text"
          :value="diagnosticText"
          readonly
          data-test="auth-diagnostic-text"
          :aria-label="t('auth.diagnostic.title')"
        />
      </section>

      <div class="auth-language">
        <span>{{ t("auth.language") }}</span>
        <NSelect :value="language" :options="languageOptions" size="small" @update:value="changeLanguage" />
      </div>
    </NCard>
  </main>
</template>

<style scoped src="./AuthGate.css"></style>
