import type { Ref } from "vue";
import type { AssistantStreamBlock, ChatMessage } from "../../../types/app";
import { assistantContentBlocksFromMessage, assistantTextFromStreamBlocks } from "../../../utils/chat-message-semantics";
import { assistantMessageHasCanonicalVisibleContent } from "./chat-message-state-machine";
import type { RoundState } from "./use-chat-flow-types";
import { readMessagePlainText } from "./use-chat-flow-utils";

type UseChatFlowStopOptions = {
  chatting: Ref<boolean>;
  submitPending?: Ref<boolean>;
  allMessages: Ref<ChatMessage[]>;
  getSession: () => { apiConfigId: string; agentId: string } | null;
  getConversationId?: () => string;
  invokeStopChatMessage?: (input: {
    session: { apiConfigId: string; agentId: string; conversationId?: string };
    partialAssistantText: string;
    partialStreamBlocks: AssistantStreamBlock[];
  }) => Promise<{
    aborted: boolean;
    persisted: boolean;
    conversationId?: string | null;
    assistantText?: string;
    assistantMessage?: ChatMessage;
  }>;
  getRound: () => RoundState;
  setRound: (next: RoundState) => void;
  advanceGeneration: () => void;
  setSendChatActiveGen: (gen: number) => void;
  clearDeferredRoundCompletion: () => void;
  clearPendingTerminalEvent: () => void;
  setActiveActivationId: (value: string) => void;
  getActiveActivationId: () => string;
  setActiveRoundAgentId: (value: string) => void;
  clearFrontendDispatchTimer: () => void;
  getPendingUserDraftId: () => string;
  removeMessage: (messageId: string) => void;
  settleStreamingAssistantMessages: () => string[];
  finalizeMessage: (messageId: string, finalMessage?: ChatMessage) => void;
  updateMessageText: (
    messageId: string,
    rawBlocks?: AssistantStreamBlock[],
    updateOptions?: { preserveActivityProjection?: boolean },
    runtimeStatus?: { toolStatusText?: string; toolStatusState?: string },
  ) => void;
  deleteSendStartedAtMs: (gen: number) => void;
  clearConversationStreamCache: (conversationId?: string | null) => void;
  reasoningStartedAtMs: Ref<number>;
  // 提交期停止时该轮次还没有任何终态事件，需标记这次发送，避免 sendChat 返回后复活轮次。
  getSendChatActiveGen?: () => number;
  markSendStopped?: (gen: number) => void;
  // 停止前冲刷流式文本缓冲，避免最后 100ms 的正文/思维链内容丢失。
  flushStreamTextBuffer: () => void;
};

function stringifyStopError(error: unknown): string {
  return error instanceof Error
    ? `${error.message}\n${error.stack || ""}`.trim()
    : (() => {
        try {
          return JSON.stringify(error);
        } catch {
          return String(error);
        }
      })();
}

export function useChatFlowStop(options: UseChatFlowStopOptions) {
  function resultMessageForStoppedRound(
    messageId: string,
    result: {
      assistantText?: string;
      assistantMessage?: ChatMessage;
    },
  ): ChatMessage | undefined {
    const responseMessage = result.assistantMessage;
    if (
      responseMessage
      && String(responseMessage.id || "").trim() === messageId
      && assistantMessageHasCanonicalVisibleContent(responseMessage)
    ) {
      return responseMessage;
    }
    const assistantText = String(result.assistantText || "").trim();
    if (!assistantText || !messageId) return undefined;
    const existing = options.allMessages.value.find((message) => String(message?.id || "").trim() === messageId);
    const providerMeta = { ...((existing?.providerMeta || {}) as Record<string, unknown>) };
    delete providerMeta._streaming;
    delete providerMeta._preStreamingStatusText;
    delete providerMeta._toolStatusText;
    delete providerMeta._toolStatusState;
    return {
      ...(existing || {}),
      id: messageId,
      role: "assistant",
      createdAt: existing?.createdAt || new Date().toISOString(),
      speakerAgentId: existing?.speakerAgentId,
      parts: [{ type: "text", text: assistantText }],
      providerMeta,
    } as ChatMessage;
  }

  async function finishLocalStoppedRound() {
    const round = options.getRound();
    const messageId = round.phase === "streaming" || round.phase === "queued" ? round.messageId : "";
    const activationId = options.getActiveActivationId();
    const activeMessage = messageId
      ? options.allMessages.value.find((message) => String(message?.id || "").trim() === messageId)
      : undefined;
    const currentStreamBlocks = assistantContentBlocksFromMessage(activeMessage);
    if (round.phase === "streaming" && messageId && currentStreamBlocks.length > 0) {
      // 先把尚未投影的最后一段内容写入消息，再统一结束所有忙碌投影。
      options.updateMessageText(messageId, currentStreamBlocks);
    }
    options.advanceGeneration();
    options.setSendChatActiveGen(0);
    options.clearDeferredRoundCompletion();
    options.clearPendingTerminalEvent();
    options.setActiveActivationId("");
    options.setActiveRoundAgentId("");
    options.clearFrontendDispatchTimer();

    const pendingUserDraftId = options.getPendingUserDraftId();
    if (pendingUserDraftId) {
      options.removeMessage(pendingUserDraftId);
    }

    if (round.phase === "streaming" || round.phase === "queued") {
      options.deleteSendStartedAtMs(round.gen);
    }
    // 当前轮次必须先按同一正式消息 ID 冻结。停止命令只负责中断，
    // 后台的正式落盘结果会异步回到前台；不能先清空内容块再等待它。
    if (messageId) {
      options.finalizeMessage(messageId);
    }
    // 再收束历史残留的忙碌气泡，避免它们在停止后重新显示为流式。
    // 当前消息已去掉 _streaming，不会在这里被二次清理。
    options.settleStreamingAssistantMessages();

    options.setRound({ phase: "idle" });
    options.chatting.value = false;
    if (options.submitPending) {
      options.submitPending.value = false;
    }
    options.reasoningStartedAtMs.value = 0;
    options.clearConversationStreamCache(options.getConversationId ? options.getConversationId() : "");
    return { messageId, activationId };
  }

  async function stopChat() {
    // 先冲刷流式文本缓冲，让最后一段正文进入消息状态，再冻结轮次。
    options.flushStreamTextBuffer();
    const round = options.getRound();
    // 提交期（发送请求尚未返回）也要能打断：这段时间轮次还没建立，
    // 但后端会给该会话留下停止意图，由随后开始的这一轮消费。
    // 这里不设任何前置门槛：前端无法可靠判断后端此刻在不在跑，
    // 只要用户按了停止就把指令交出去，由后端决定中止哪一轮。
    const submitPending = !!options.submitPending?.value;

    const stopSession = options.getSession();
    const cid = options.getConversationId ? options.getConversationId() : "";
    const activeMessageId = round.phase === "streaming" || round.phase === "queued" ? round.messageId : "";
    const activeMessage = activeMessageId
      ? options.allMessages.value.find((message) => String(message?.id || "") === activeMessageId)
      : undefined;
    const partialStreamBlocks = assistantContentBlocksFromMessage(activeMessage);
    const partialAssistantText = readMessagePlainText(activeMessage)
      || assistantTextFromStreamBlocks(partialStreamBlocks);

    // 提交期停止时该轮次尚未建立，sendChat 返回后会重新 setRound({phase:"queued"})。
    // 先记下这次发送的 gen，sendChat 侧据此跳过轮次复活。
    if (submitPending) {
      const stoppedGen = options.getSendChatActiveGen ? options.getSendChatActiveGen() : 0;
      if (stoppedGen) options.markSendStopped?.(stoppedGen);
    }

    // 先立即结束本地忙碌态，再通知后端；后端有同一消息的正式结果才回写。
    const stoppedRound = await finishLocalStoppedRound();

    if (stopSession && options.invokeStopChatMessage) {
      try {
        const result = await options.invokeStopChatMessage({
          session: cid ? { ...stopSession, conversationId: cid } : stopSession,
          partialAssistantText,
          partialStreamBlocks,
        });
        const finalMessage = resultMessageForStoppedRound(stoppedRound.messageId, result);
        if (finalMessage) {
          options.finalizeMessage(stoppedRound.messageId, finalMessage);
        }
      } catch (error) {
        const et = stringifyStopError(error);
        console.warn(`[聊天] 停止消息失败，apiConfigId=${stopSession.apiConfigId}，agentId=${stopSession.agentId}，len=${partialAssistantText.length}，错误=${et}`);
      }
    }
  }

  return {
    stopChat,
  };
}
