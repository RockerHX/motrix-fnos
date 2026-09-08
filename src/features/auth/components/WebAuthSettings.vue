<script setup lang="ts">
import { computed, nextTick, reactive, ref } from "vue";
import { NAlert, NButton, NForm, NFormItem, NInput, NModal, NSpace, NText, useMessage, type InputInst } from "naive-ui";
import { getErrorMessage } from "../../../app/utils/errors";
import { useI18n } from "../../../i18n";
import { useAuthStore } from "../stores/authStore";

const authStore = useAuthStore();
const message = useMessage();
const { t } = useI18n();
const showPasswordModal = ref(false);
const passwordInput = ref<InputInst | null>(null);
const passwordError = ref("");
const passwordForm = reactive({ currentPassword: "", newPassword: "", confirmPassword: "" });
const canManage = computed(() => authStore.authenticated);
const securityModalStyle = {
  width: "min(520px, calc(100vw - 32px))",
  maxWidth: "calc(100vw - 32px)",
};

function openPasswordModal() {
  if (!canManage.value) return;
  resetPasswordForm();
  showPasswordModal.value = true;
  void focusInput(passwordInput);
}

async function submitPasswordChange() {
  passwordError.value = validatePasswordChange();
  if (passwordError.value) {
    await focusInput(passwordInput);
    return;
  }
  try {
    await authStore.changePassword({
      currentPassword: passwordForm.currentPassword,
      newPassword: passwordForm.newPassword,
    });
    message.success(t("auth.security.passwordChanged"));
    showPasswordModal.value = false;
    resetPasswordForm();
  } catch (error) {
    passwordError.value = getErrorMessage(error, t("auth.security.operationFailed"));
    await focusInput(passwordInput);
  }
}

function validatePasswordChange() {
  if (!passwordForm.currentPassword || !passwordForm.newPassword) return t("auth.passwordRequired");
  const charCount = Array.from(passwordForm.newPassword).length;
  const byteCount = new TextEncoder().encode(passwordForm.newPassword).length;
  if (charCount < 8 || charCount > 128 || byteCount > 512) return t("auth.passwordLength");
  if (passwordForm.newPassword !== passwordForm.confirmPassword) return t("auth.passwordMismatch");
  return "";
}

function resetPasswordForm() {
  passwordForm.currentPassword = "";
  passwordForm.newPassword = "";
  passwordForm.confirmPassword = "";
  passwordError.value = "";
}

async function focusInput(target: typeof passwordInput) {
  await nextTick();
  target.value?.focus();
}
</script>

<template>
  <section class="auth-security" data-test="web-auth-settings">
    <div class="auth-security-heading">
      <div>
        <h3>{{ t("auth.security.title") }}</h3>
        <NText depth="3">{{ t("auth.security.description") }}</NText>
      </div>
      <NText type="success">{{ t("auth.security.enabled") }}</NText>
    </div>

    <NAlert v-if="!canManage" type="info" :bordered="false">
      {{ t("auth.security.adminRequired") }}
    </NAlert>

    <div class="auth-security-row">
      <div>
        <strong>{{ t("auth.security.password") }}</strong>
        <p>{{ t("auth.security.passwordHelp") }}</p>
      </div>
      <NButton :disabled="!canManage" @click="openPasswordModal">{{ t("auth.security.changePassword") }}</NButton>
    </div>

    <NModal
      v-model:show="showPasswordModal"
      preset="card"
      class="auth-security-modal"
      :style="securityModalStyle"
      :title="t('auth.security.changePassword')"
      :mask-closable="!authStore.isSubmitting"
      :closable="!authStore.isSubmitting"
      @after-leave="resetPasswordForm"
    >
      <NForm @submit.prevent="submitPasswordChange">
        <NAlert v-if="passwordError" type="error" data-test="password-error">{{ passwordError }}</NAlert>
        <NFormItem :label="t('auth.security.currentPassword')">
          <NInput
            ref="passwordInput"
            v-model:value="passwordForm.currentPassword"
            type="password"
            :input-props="{ autocomplete: 'current-password' }"
          />
        </NFormItem>
        <NFormItem :label="t('auth.security.newPassword')">
          <NInput v-model:value="passwordForm.newPassword" type="password" :input-props="{ autocomplete: 'new-password' }" />
        </NFormItem>
        <NFormItem :label="t('auth.passwordConfirm')">
          <NInput v-model:value="passwordForm.confirmPassword" type="password" :input-props="{ autocomplete: 'new-password' }" />
        </NFormItem>
        <NSpace justify="end">
          <NButton :disabled="authStore.isSubmitting" @click="showPasswordModal = false">{{ t("common.cancel") }}</NButton>
          <NButton type="primary" attr-type="submit" :loading="authStore.isSubmitting">
            {{ t("common.save") }}
          </NButton>
        </NSpace>
      </NForm>
    </NModal>

  </section>
</template>

<style scoped src="./WebAuthSettings.css"></style>
