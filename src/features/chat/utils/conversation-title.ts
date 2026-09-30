import type { ChatConversationOverviewItem } from "../../../types/app";

type ConversationTitleLike = Pick<
  ChatConversationOverviewItem,
  "conversationId" | "kind" | "title" | "summaryTitle" | "remoteContactDisplayName" | "updatedAt" | "lastMessageAt" | "isSystemNotificationConversation"
>;

type ResolveConversationDisplayTitleOptions = {
  locale?: string;
  untitledLabel: string;
  /** 系统通知会话的标题；缺省时回退到中文常量，用于没有 i18n 上下文的调用方 */
  systemNotificationLabel?: string;
};

function normalizedTitlePart(value?: string, conversationId?: string): string {
  const title = String(value || "").trim();
  if (!title) return "";
  const normalizedConversationId = String(conversationId || "").trim();
  if (normalizedConversationId && title === normalizedConversationId) return "";
  return title;
}

export const SYSTEM_NOTIFICATION_DISPLAY_TITLE = "系统通知";

export function formatConversationFallbackTitle(value?: string, locale?: string): string {
  const rawValue = normalizedTitlePart(value);
  if (!rawValue) return "";
  const date = new Date(rawValue);
  if (Number.isNaN(date.getTime())) return rawValue;
  const now = new Date();
  const sameYear = date.getFullYear() === now.getFullYear();
  const dateText = date.toLocaleDateString(locale || undefined, sameYear
    ? {
      month: "2-digit",
      day: "2-digit",
    }
    : {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    });
  const timeText = date.toLocaleTimeString(locale || undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });
  return `${dateText} ${timeText}`.trim();
}

export function resolveConversationDisplayTitle(
  item: ConversationTitleLike,
  options: ResolveConversationDisplayTitleOptions,
): string {
  if (item.isSystemNotificationConversation) {
    return String(options.systemNotificationLabel || "").trim() || SYSTEM_NOTIFICATION_DISPLAY_TITLE;
  }
  if (item.kind === "remote_im_contact") {
    return normalizedTitlePart(item.remoteContactDisplayName)
      || normalizedTitlePart(item.title, item.conversationId)
      || options.untitledLabel;
  }
  return normalizedTitlePart(item.title, item.conversationId)
    || normalizedTitlePart(item.summaryTitle, item.conversationId)
    || formatConversationFallbackTitle(item.lastMessageAt || item.updatedAt, options.locale)
    || options.untitledLabel;
}
