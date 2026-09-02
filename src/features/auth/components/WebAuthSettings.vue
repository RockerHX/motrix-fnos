<script setup lang="ts">
import { computed, nextTick, reactive, ref } from "vue";
import { NAlert, NButton, NForm, NFormItem, NInput, NModal, NSpace, NSwitch, NText, useMessage, type InputInst } from "naive-ui";
import { getErrorMessage } from "../../../app/utils/errors";
import { useI18n } from "../../../i18n";
import { useAuthStore } from "../stores/authStore";

const authStore = useAuthStore();
const message = useMessage();
const { t } = useI18n();
const showPasswordModal = ref(false);
const showProtectionModal = ref(false);
const passwordInput = ref<InputInst | null>(null);
const protectionPasswordInput = ref<InputInst | null>(null);
const passwordError = ref("");
const protectionError = ref("");
const requestedProtection = ref(true);
const passwordForm = reactive({ currentPassword: "", newPassword: "", confirmPassword: "" });
const protectionForm = reactive({ currentPassword: "" });
const canManage = computed(() => authStore.authenticated || !authStore.enabled);
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

function requestProtectionChange(enabled: boolean) {
  if (!canManage.value || enabled === authStore.enabled) return;
  requestedProtection.value = enabled;
  protectionForm.currentPassword = "";
  protectionError.value = "";
  showProtectionModal.value = true;
  void focusInput(protectionPasswordInput);
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

async function submitProtectionChange() {
  if (!protectionForm.currentPassword) {
    protectionError.value = t("auth.passwordRequired");
    await focusInput(protectionPasswordInput);
    return;
  }
  try {
    await authStore.setProtection(requestedProtection.value, protectionForm.currentPassword);
    message.success(t(requestedProtection.value ? "auth.security.protectionEnabled" : "auth.security.protectionDisabled"));
    showProtectionModal.value = false;
    protectionForm.currentPassword = "";
    protectionError.value = "";
  } catch (error) {
    protectionError.value = getErrorMessage(error, t("auth.security.operationFailed"));
    await focusInput(protectionPasswordInput);
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

function resetProtectionForm() {
  protectionForm.currentPassword = "";
  protectionError.value = "";
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
      <NText :type="authStore.enabled ? 'success' : 'warning'">
        {{ t(authStore.enabled ? "auth.security.enabled" : "auth.security.disabled") }}
      </NText>
    </div>

    <NAlert v-if="!authStore.enabled" type="warning" :title="t('auth.security.riskTitle')" :bordered="false">
      {{ t("auth.security.riskDescription") }}
    </NAlert>
    <NAlert v-if="!canManage" type="info" :bordered="false">
      {{ t("auth.security.adminRequired") }}
    </NAlert>

    <div class="auth-security-row">
      <div>
        <strong>{{ t("auth.security.protection") }}</strong>
        <p>{{ t("auth.security.protectionHelp") }}</p>
      </div>
      <NSwitch
        :value="authStore.enabled"
        :disabled="!canManage || authStore.isSubmitting"
        :loading="authStore.isSubmitting"
        data-test="auth-protection-switch"
        @update:value="requestProtectionChange"
      />
    </div>

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

    <NModal
      v-model:show="showProtectionModal"
      preset="card"
      class="auth-security-modal"
      :style="securityModalStyle"
      :title="t(requestedProtection ? 'auth.security.enableTitle' : 'auth.security.disableTitle')"
      :mask-closable="!authStore.isSubmitting"
      :closable="!authStore.isSubmitting"
      @after-leave="resetProtectionForm"
    >
      <NForm @submit.prevent="submitProtectionChange">
        <NAlert :type="requestedProtection ? 'info' : 'warning'" :title="t(requestedProtection ? 'auth.security.enableTitle' : 'auth.security.disableTitle')">
          {{ t(requestedProtection ? "auth.security.enableConfirm" : "auth.security.disableConfirm") }}
        </NAlert>
        <NAlert v-if="protectionError" type="error" data-test="protection-error">{{ protectionError }}</NAlert>
        <NFormItem :label="t('auth.security.currentPassword')">
          <NInput
            ref="protectionPasswordInput"
            v-model:value="protectionForm.currentPassword"
            type="password"
            :input-props="{ autocomplete: 'current-password' }"
          />
        </NFormItem>
        <NSpace justify="end">
          <NButton :disabled="authStore.isSubmitting" @click="showProtectionModal = false">{{ t("common.cancel") }}</NButton>
          <NButton :type="requestedProtection ? 'primary' : 'error'" attr-type="submit" :loading="authStore.isSubmitting">
            {{ t(requestedProtection ? "auth.security.enable" : "auth.security.disable") }}
          </NButton>
        </NSpace>
      </NForm>
    </NModal>
  </section>
</template>

<style scoped src="./WebAuthSettings.css"></style>
