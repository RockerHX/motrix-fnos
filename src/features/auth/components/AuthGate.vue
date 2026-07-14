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
import { language, setLanguage, supportedLanguages, useI18n, type AppLanguage } from "../../../i18n";
import { useAuthStore } from "../stores/authStore";

const LANGUAGE_KEY = "motrix-fnos:language";
const authStore = useAuthStore();
const { t } = useI18n();
const passwordInput = ref<InputInst | null>(null);
const submitError = ref("");
const form = reactive({ password: "", confirmPassword: "" });
const languageOptions = computed(() =>
  supportedLanguages.map((value) => ({
    value,
    label: value === "zh-CN" ? t("language.zhCN") : t("language.enUS"),
  })),
);

const isSetup = computed(() => authStore.phase === "setup");
const title = computed(() => t(isSetup.value ? "auth.setup.title" : "auth.login.title"));
const description = computed(() => t(isSetup.value ? "auth.setup.description" : "auth.login.description"));

onMounted(() => {
  const saved = localStorage.getItem(LANGUAGE_KEY);
  if (supportedLanguages.includes(saved as AppLanguage)) {
    setLanguage(saved);
  }
  void focusPassword();
});

async function submit() {
  if (authStore.isSubmitting || (authStore.phase !== "setup" && authStore.phase !== "login")) return;
  submitError.value = validateForm();
  if (submitError.value) {
    await focusPassword();
    return;
  }
  try {
    if (isSetup.value) {
      await authStore.setup(form.password);
    } else {
      await authStore.login(form.password);
    }
    form.password = "";
    form.confirmPassword = "";
  } catch (error) {
    submitError.value = getErrorMessage(error, t("auth.submitFailed"));
    await focusPassword();
  }
}

function validateForm() {
  if (!form.password) return t("auth.passwordRequired");
  const charCount = Array.from(form.password).length;
  const byteCount = new TextEncoder().encode(form.password).length;
  if (charCount < 12 || charCount > 128 || byteCount > 512) return t("auth.passwordLength");
  if (isSetup.value && form.password !== form.confirmPassword) return t("auth.passwordMismatch");
  return "";
}

function changeLanguage(value: AppLanguage) {
  setLanguage(value);
  localStorage.setItem(LANGUAGE_KEY, value);
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

      <NForm v-else class="auth-form" :show-label="true" @submit.prevent="submit">
        <div class="auth-heading">
          <h1>{{ title }}</h1>
          <p>{{ description }}</p>
        </div>
        <NAlert v-if="submitError" type="error" data-test="auth-submit-error">{{ submitError }}</NAlert>
        <NFormItem :label="t('auth.password')">
          <NInput
            ref="passwordInput"
            v-model:value="form.password"
            type="password"
            show-password-on="mousedown"
            :placeholder="t('auth.passwordPlaceholder')"
            :input-props="{ autocomplete: isSetup ? 'new-password' : 'current-password' }"
            :disabled="authStore.isSubmitting"
            data-test="auth-password"
          />
        </NFormItem>
        <NFormItem v-if="isSetup" :label="t('auth.passwordConfirm')">
          <NInput
            v-model:value="form.confirmPassword"
            type="password"
            show-password-on="mousedown"
            :input-props="{ autocomplete: 'new-password' }"
            :disabled="authStore.isSubmitting"
            data-test="auth-password-confirm"
          />
        </NFormItem>
        <NButton block type="primary" attr-type="submit" :loading="authStore.isSubmitting" data-test="auth-submit">
          {{ t(isSetup ? "auth.setup.submit" : "auth.login.submit") }}
        </NButton>
      </NForm>

      <div class="auth-language">
        <span>{{ t("auth.language") }}</span>
        <NSelect :value="language" :options="languageOptions" size="small" @update:value="changeLanguage" />
      </div>
    </NCard>
  </main>
</template>

<style scoped>
.auth-gate { min-height: var(--app-viewport-height); display: grid; place-items: center; padding: max(16px, env(safe-area-inset-top)) 16px max(16px, env(safe-area-inset-bottom)); background: radial-gradient(circle at top, #202820 0, #151515 45%); overflow-y: auto; }
.auth-card { width: min(100%, 430px); background: #1b1f1d; box-shadow: 0 24px 60px rgb(0 0 0 / 32%); }
.auth-brand { display: flex; align-items: center; gap: 12px; margin-bottom: 28px; }
.auth-logo { width: 46px; height: 46px; border-radius: 11px; }
.auth-brand strong { color: var(--app-text-primary); font-size: 22px; }
.auth-brand p, .auth-heading p, .auth-state p { margin: 4px 0 0; color: var(--app-text-muted); line-height: 1.5; }
.auth-state { min-height: 220px; display: flex; flex-direction: column; justify-content: center; align-items: center; gap: 20px; text-align: center; }
.auth-form { display: grid; gap: 4px; }
.auth-heading { margin-bottom: 12px; }
.auth-heading h1 { margin: 0; color: var(--app-text-primary); font-size: 24px; }
.auth-language { display: flex; align-items: center; justify-content: flex-end; gap: 10px; margin-top: 22px; color: var(--app-text-muted); font-size: 13px; }
.auth-language :deep(.n-select) { width: 132px; }
@media (max-width: 480px) { .auth-gate { place-items: start center; padding-top: max(24px, env(safe-area-inset-top)); } .auth-card { margin: auto 0; } }
</style>
