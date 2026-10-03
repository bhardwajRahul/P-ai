<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri, isDesktopTauriHost, openTransportFileDialog } from "../../../../services/tauri-api";

const { t } = useI18n();

type GrokAuthStatus = {
  providerId: string;
  authenticated: boolean;
  status: string;
  message: string;
  email: string;
  accountId: string;
  fileName: string;
  filePath: string;
  expiresAt: string;
  baseUrl: string;
};

const props = defineProps<{ providerId: string }>();
const status = ref<GrokAuthStatus | null>(null);
const busy = ref(false);
const errorMessage = ref("");

async function refreshStatus() {
  if (!props.providerId) return;
  status.value = await invokeTauri<GrokAuthStatus>("grok_auth_status", {
    input: { providerId: props.providerId },
  });
}

async function login() {
  busy.value = true;
  errorMessage.value = "";
  try {
    status.value = await invokeTauri<GrokAuthStatus>("grok_auth_login", {
      input: { providerId: props.providerId },
    });
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    busy.value = false;
  }
}

async function refreshToken() {
  busy.value = true;
  errorMessage.value = "";
  try {
    status.value = await invokeTauri<GrokAuthStatus>("grok_auth_refresh", {
      input: { providerId: props.providerId },
    });
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    busy.value = false;
  }
}

async function logout() {
  busy.value = true;
  errorMessage.value = "";
  try {
    status.value = await invokeTauri<GrokAuthStatus>("grok_auth_logout", {
      input: { providerId: props.providerId },
    });
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    busy.value = false;
  }
}

async function importFile() {
  errorMessage.value = "";
  if (!isDesktopTauriHost()) {
    errorMessage.value = t("config.api.grokLoginDesktopOnly");
    return;
  }
  const picked = await openTransportFileDialog({
    title: t("config.api.grokLoginImportTitle"),
    filters: [{ name: t("config.api.grokLoginImportFilter"), extensions: ["json"] }],
  });
  const filePath = Array.isArray(picked) ? picked[0] : picked;
  if (!filePath) return;
  busy.value = true;
  try {
    status.value = await invokeTauri<GrokAuthStatus>("grok_auth_import", {
      input: { providerId: props.providerId, filePath },
    });
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    busy.value = false;
  }
}

onMounted(() => {
  void refreshStatus().catch((error) => {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  });
});

watch(() => props.providerId, () => {
  void refreshStatus().catch((error) => {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  });
});
</script>

<template>
  <section class="rounded-box border border-base-300 bg-base-100 p-4">
    <div class="flex items-center justify-between gap-3">
      <div>
        <h3 class="text-sm font-medium">{{ t("config.api.grokLoginTitle") }}</h3>
        <p class="mt-1 text-xs opacity-70">{{ status?.message || t("config.api.grokLoginIdle") }}</p>
      </div>
      <div class="flex gap-2">
        <button class="btn btn-sm" type="button" :disabled="busy" @click="login">{{ t("config.api.grokLoginAction") }}</button>
        <button class="btn btn-sm" type="button" :disabled="busy || !status?.authenticated" @click="refreshToken">{{ t("config.api.grokLoginRefresh") }}</button>
        <button class="btn btn-sm" type="button" :disabled="busy || !status?.fileName" @click="logout">{{ t("config.api.grokLoginLogout") }}</button>
      </div>
    </div>
    <dl v-if="status?.email" class="mt-3 grid grid-cols-[5rem_1fr] gap-y-1 text-xs">
      <dt class="opacity-60">{{ t("config.api.grokLoginEmail") }}</dt>
      <dd>{{ status.email }}</dd>
      <dt class="opacity-60">{{ t("config.api.grokLoginFile") }}</dt>
      <dd class="break-all">{{ status.fileName }}</dd>
      <dt class="opacity-60">{{ t("config.api.grokLoginBaseUrl") }}</dt>
      <dd class="break-all">{{ status.baseUrl }}</dd>
    </dl>
    <div class="mt-3">
      <button class="btn btn-sm" type="button" :disabled="busy" @click="importFile">{{ t("config.api.grokLoginImport") }}</button>
    </div>
    <p v-if="errorMessage" class="mt-2 text-xs text-error">{{ errorMessage }}</p>
  </section>
</template>
