import type { ChatConversationOverviewItem, ConversationPreviewMessage } from "../../../types/app";

// ==================== 会话项展示判定（纯函数，供 ChatConversationItem 与 Sidebar 共用） ====================

export type ConversationItemLevel = "full" | "sim" | "mini";

/**
 * 天起点：凌晨 4 点。当前时刻未到 4 点时，天起点回退到前一天 4 点。
 * 与「最近会话」计数使用同一套「凌晨 4 点区分天」的口径。
 */
export function activityDayStartMs(now: number = Date.now()): number {
  const dayStart = new Date(now);
  dayStart.setHours(4, 0, 0, 0);
  if (dayStart.getTime() > now) {
    dayStart.setDate(dayStart.getDate() - 1);
  }
  return dayStart.getTime();
}

/** 未读，或今天（凌晨 4 点起）有过活动 → sim（摘要常显）；否则 → mini（摘要折叠、hover 展开） */
export function simpleConversationItemLevel(
  item: ChatConversationOverviewItem,
  now: number = Date.now(),
): "sim" | "mini" {
  return hasUnreadOrRecentActivity(item, now) ? "sim" : "mini";
}

export function hasUnreadOrRecentActivity(
  item: ChatConversationOverviewItem,
  now: number = Date.now(),
): boolean {
  if (Number(item.unreadCount || 0) > 0) return true;
  const raw = String(item.lastMessageAt || item.updatedAt || "").trim();
  if (!raw) return false;
  const time = Date.parse(raw);
  if (!Number.isFinite(time)) return false;
  return time >= activityDayStartMs(now);
}

/** 未读角标：当前会话不显示；0 不显示；超过 99 显示 99+ */
export function conversationUnreadBadge(
  item: ChatConversationOverviewItem,
  activeConversationId: string,
): string {
  if (String(item.conversationId || "").trim() === String(activeConversationId || "").trim()) {
    return "";
  }
  const unreadCount = Math.max(0, Number(item.unreadCount || 0));
  if (unreadCount <= 0) return "";
  return unreadCount > 99 ? "99+" : String(unreadCount);
}

/** 运行时是否忙碌（流式/整理上下文/归档/压缩） */
export function conversationRuntimeBusy(runtimeState?: ChatConversationOverviewItem["runtimeState"]): boolean {
  return runtimeState === "assistant_streaming"
    || runtimeState === "organizing_context"
    || runtimeState === "archiving"
    || runtimeState === "compacting";
}

