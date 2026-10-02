<template>
  <div class="rounded-box border border-base-300 bg-base-100 px-3 py-3">
    <div class="text-xs opacity-70">{{ t("chat.selection.selectedCount", { count: selectedMessageCount }) }}</div>
    <div class="mt-3 flex flex-wrap items-center gap-2">
      <button type="button" class="btn btn-sm" :disabled="selectedMessageCount === 0" @click="emit('selectionActionBranch')">
        {{ t("chat.selection.branch") }}
      </button>
      <button
        v-if="showConversationActions"
        type="button"
        class="btn btn-sm"
        :class="{ 'btn-primary': selectionDeliverCardOpen }"
        :disabled="selectedMessageCount === 0 || selectionDeliverTargetOptions.length === 0"
        @click="openSelectionDeliverCard"
      >
        {{ t("chat.selection.forward") }}
      </button>
      <button type="button" class="btn btn-sm" :disabled="selectedMessageCount === 0" @click="emit('selectionActionCopy')">
        {{ t("common.copy") }}
      </button>
      <button v-if="showConversationActions" type="button" class="btn btn-sm" :disabled="selectedMessageCount === 0" @click="emit('selectionActionShare', 'copyPng')">
        {{ t("chat.selection.copyImageAsImage") }}
      </button>
      <button v-if="showConversationActions" type="button" class="btn btn-sm" :disabled="selectedMessageCount === 0" @click="emit('selectionActionShare', 'png')">
        {{ t("chat.selection.saveAsImage") }}
      </button>
      <button v-if="showConversationActions" type="button" class="btn btn-sm" :disabled="selectedMessageCount === 0" @click="emit('selectionActionShare', 'html')">
        {{ t("chat.selection.saveAsHtml") }}
      </button>
      <button type="button" class="btn btn-sm btn-ghost ml-auto" @click="emit('exitSelectionMode')">
        {{ t("common.cancel") }}
      </button>
    </div>

    <div v-if="showConversationActions && selectionDeliverCardOpen" class="mt-3 rounded-box border border-base-300 bg-base-200/50 px-3 py-3">
      <div class="text-sm font-medium">{{ t("chat.selection.forward") }}</div>
      <div class="mt-1 text-xs opacity-70">{{ t("chat.selection.forwardHint") }}</div>
      <select v-model="selectionDeliverTargetKey" class="select select-bordered select-sm mt-3 w-full" :disabled="selectionDeliverTargetOptions.length === 0">
        <option v-for="item in selectionDeliverTargetOptions" :key="item.targetKey" :value="item.targetKey">
          {{ selectionDeliverOptionLabel(item) }}
        </option>
      </select>
      <div class="mt-3 flex items-center justify-end gap-2">
        <button type="button" class="btn btn-sm" @click="closeSelectionDeliverCard">{{ t("common.cancel") }}</button>
        <button type="button" class="btn btn-sm btn-primary" :disabled="!selectionDeliverTargetKey" @click="confirmSelectionDeliver">
          {{ t("chat.selection.confirmForward") }}
        </button>
      </div>
    </div>

  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { ChatConversationOverviewItem, ConversationForwardTarget, RemoteImContactConversationOption } from "../../../types/app";
import { resolveConversationDisplayTitle } from "../utils/conversation-title";

const props = defineProps<{
  showConversationActions?: boolean;
  selectedMessageCount: number;
  activeConversationId: string;
  unarchivedConversationItems: ChatConversationOverviewItem[];
  remoteImContactConversations: RemoteImContactConversationOption[];
}>();

const emit = defineEmits<{
  exitSelectionMode: [];
  selectionActionBranch: [];
  selectionActionForward: [target: ConversationForwardTarget];
  selectionActionCopy: [];
  selectionActionShare: [format: "html" | "png" | "copyPng"];
}>();

const { t, locale } = useI18n();
const showConversationActions = computed(() => props.showConversationActions ?? true);

const selectionDeliverCardOpen = ref(false);
const selectionDeliverTargetKey = ref("");

const selectionDeliverTargetOptions = computed(() => {
  const activeConversationId = String(props.activeConversationId || "").trim();
  const localTargets = (Array.isArray(props.unarchivedConversationItems) ? props.unarchivedConversationItems : [])
    .filter((item) => String(item.conversationId || "").trim() !== activeConversationId)
    .filter((item) => !item.isSystemNotificationConversation)
    .filter((item) => String(item.kind || "local_unarchived").trim() !== "remote_im_contact")
    .map((item) => ({
      targetKey: `local:${String(item.conversationId || "").trim()}`,
      target: {
        kind: "local_unarchived" as const,
        conversationId: String(item.conversationId || "").trim(),
      },
      title: resolveConversationDisplayTitle(item, {
        locale: locale.value,
        untitledLabel: t("chat.untitledConversation"),
        systemNotificationLabel: t("chat.systemPersona"),
      }),
      runtimeState: item.runtimeState,
    }))
    .filter((item) => !!item.target.conversationId);
  const remoteTargets = (Array.isArray(props.remoteImContactConversations) ? props.remoteImContactConversations : [])
    .filter((item) => String(item.conversationId || "").trim() !== activeConversationId)
    .map((item) => ({
      targetKey: `remote:${String(item.contactId || "").trim()}:${String(item.conversationId || "").trim()}`,
      target: {
        kind: "remote_im_contact" as const,
        conversationId: String(item.conversationId || "").trim(),
        remoteContactId: String(item.contactId || "").trim(),
      },
      title: String(item.title || "").trim() || String(item.contactDisplayName || "").trim(),
      remoteContactName: String(item.contactDisplayName || "").trim() || undefined,
      channelName: String(item.channelName || "").trim() || undefined,
    }))
    .filter((item) => !!item.target.conversationId && !!item.target.remoteContactId);
  return [...localTargets, ...remoteTargets];
});

function selectionDeliverOptionLabel(item: {
  target: ConversationForwardTarget;
  title: string;
  runtimeState?: ChatConversationOverviewItem["runtimeState"];
  remoteContactName?: string;
  channelName?: string;
}): string {
  const parts = [String(item.title || "").trim() || t('chat.selection.unnamedConversation')];
  if (item.target.kind === "remote_im_contact") {
    const remoteContactName = String(item.remoteContactName || "").trim();
    const channelName = String(item.channelName || "").trim();
    if (remoteContactName) parts.push(remoteContactName);
    if (channelName) parts.push(channelName);
  } else {
    if (item.runtimeState === "assistant_streaming") parts.push(t('chat.selection.streaming'));
    if (item.runtimeState === "organizing_context") parts.push(t('chat.selection.organizing'));
  }
  return parts.join(" / ");
}

function openSelectionDeliverCard() {
  if (selectionDeliverTargetOptions.value.length === 0) return;
  const currentTargetKey = String(selectionDeliverTargetKey.value || "").trim();
  const hasValidTarget = selectionDeliverTargetOptions.value.some((item) => item.targetKey === currentTargetKey);
  if (!currentTargetKey || !hasValidTarget) {
    selectionDeliverTargetKey.value = selectionDeliverTargetOptions.value[0]?.targetKey || "";
  }
  selectionDeliverCardOpen.value = true;
}

function closeSelectionDeliverCard() {
  selectionDeliverCardOpen.value = false;
}

function confirmSelectionDeliver() {
  const targetKey = String(selectionDeliverTargetKey.value || "").trim();
  const target = selectionDeliverTargetOptions.value.find((item) => item.targetKey === targetKey)?.target;
  if (!target?.conversationId) return;
  closeSelectionDeliverCard();
  emit("selectionActionForward", target);
}
</script>
