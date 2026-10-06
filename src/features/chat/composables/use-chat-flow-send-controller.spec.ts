import { ref } from "vue";
import { describe, expect, it, vi } from "vitest";
import type { PreparedChatSendInput } from "./use-chat-flow-send-input";
import type { RoundState } from "./use-chat-flow-types";
import { buildChatIngressParts, useChatFlowSendController } from "./use-chat-flow-send-controller";

function buildSendHarness(options?: {
  isSendStopped?: (gen: number) => boolean;
  submitPendingValue?: boolean;
}) {
  const chatting = ref(false);
  const submitPending = ref(options?.submitPendingValue ?? false);
  let round: RoundState = { phase: "idle" };
  let gen = 0;
  const prepared: PreparedChatSendInput = {
    useOverrideMessage: false,
    plainText: "你好",
    displayText: "你好",
    selectedMentions: [],
    extraTextBlocks: [],
    sentImages: [],
    attachments: [],
    sendSession: { apiConfigId: "api-1", agentId: "agent-1" } as PreparedChatSendInput["sendSession"],
    sendConversationId: "conversation-1",
  };
  const setRound = vi.fn((next: RoundState) => {
    round = next;
  });
  const insertUserDraft = vi.fn(() => "user-1");
  const onStreamingAssistantBubbleInserted = vi.fn();
  const updateQueuedAssistantMessageStatus = vi.fn();
  const invokeSendChatMessage = vi.fn(async () => ({
    accepted: true,
    duplicate: false,
    eventId: "event-1",
    conversationId: "conversation-1",
    traceId: "trace-1",
    ingress: "started",
    userMessageId: "user-1",
    assistantMessageId: "assistant-1",
  }));

  const { sendChat } = useChatFlowSendController({
    chatting,
    submitPending,
    getConversationId: () => "conversation-1",
    getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
    createSendChatDeltaChannel: () => ({}) as never,
    invokeSendChatMessage,
    onOwnUserDraftInserted: vi.fn(),
    onStreamingAssistantBubbleInserted,
    t: (key: string) => key,
    getRound: () => round,
    setRound,
    nextGeneration: () => ++gen,
    setSendChatActiveGen: vi.fn(),
    setActiveActivationId: vi.fn(),
    setActiveRoundAgentId: vi.fn(),
    setPendingTerminalEventNull: vi.fn(),
    sendStartedAtMsByGen: new Map<number, number>(),
    startFrontendDispatchTimer: vi.fn(),
    clearFrontendDispatchTimer: vi.fn(),
    clearConversationStreamCache: vi.fn(),
    clearChatErrorText: vi.fn(),
    applyPreparedSendInput: vi.fn(),
    prepareSendInput: () => prepared,
    insertUserDraft,
    resetDisplayState: vi.fn(),
    removeMessage: vi.fn(),
    updateQueuedAssistantMessageStatus,
    handleRoundCompleted: vi.fn(async () => {}),
    sendRecovery: {
      handleAbortedSend: vi.fn(),
      handleFailedSend: vi.fn(async () => {}),
      finalizeSendChat: vi.fn(async () => {}),
    },
    isConversationBusy: () => false,
    isSendStopped: options?.isSendStopped,
    clearSendStopped: vi.fn(),
  });

  return {
    sendChat,
    submitPending,
    setRound,
    insertUserDraft,
    onStreamingAssistantBubbleInserted,
    updateQueuedAssistantMessageStatus,
  };
}

describe("useChatFlowSendController", () => {

describe("buildChatIngressParts", () => {
  it("builds ordered canonical parts without legacy attachment mirrors", () => {
    const parts = buildChatIngressParts(
      "请处理附件",
      [
        {
          mime: "image/png",
          bytesBase64: "ignored-because-path-is-authoritative",
          savedPath: "C:\\workspace\\downloads\\source.png",
        },
        {
          mime: "image/jpeg",
          bytesBase64: "raw-image",
        },
      ],
      [
        {
          fileName: "source.png",
          path: "C:/workspace/downloads/source.png",
          mime: "image/png",
        },
        {
          fileName: "report.pdf",
          path: "C:/workspace/downloads/report.pdf",
          mime: "application/pdf",
        },
      ],
    );

    expect(parts).toEqual([
      { type: "text", text: "请处理附件" },
      {
        type: "attachment",
        path: "C:/workspace/downloads/source.png",
        mime: "image/png",
        name: "source.png",
      },
      {
        type: "attachment",
        bytesBase64: "raw-image",
        mime: "image/jpeg",
        name: "image",
      },
      {
        type: "attachment",
        path: "C:/workspace/downloads/report.pdf",
        mime: "application/pdf",
        name: "report.pdf",
      },
    ]);
    expect(JSON.stringify(parts)).not.toContain("relativePath");
    expect(JSON.stringify(parts)).not.toContain("attachments");
  });
});

  it("submits the assistant round when the send was not stopped", async () => {
    const harness = buildSendHarness({ submitPendingValue: true });
    await harness.sendChat();
    expect(harness.setRound).toHaveBeenCalledWith(expect.objectContaining({ phase: "queued" }));
    expect(harness.onStreamingAssistantBubbleInserted).toHaveBeenCalledOnce();
  });

  it("skips reviving the round when the send was stopped during the submit window", async () => {
    const harness = buildSendHarness({
      submitPendingValue: true,
      isSendStopped: (gen) => gen === 1,
    });
    await harness.sendChat();
    expect(harness.setRound).not.toHaveBeenCalledWith(expect.objectContaining({ phase: "queued" }));
    expect(harness.onStreamingAssistantBubbleInserted).not.toHaveBeenCalled();
    expect(harness.updateQueuedAssistantMessageStatus).not.toHaveBeenCalled();
    // 用户自己的消息仍要落到会话里，只跳过助理轮次。
    expect(harness.insertUserDraft).toHaveBeenCalledOnce();
  });
});
