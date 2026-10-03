/**
 * 会话悬浮操作区（ChatWorkspaceToolbar）背靠弹性空间判定。
 *
 * 当视口下方存在弹性留白（latestOwnTailSpacerMinHeight > 0）时，若真实消息内容已在
 * 操作栏上沿之上结束，操作栏所遮挡的区域背后完全是纯空白（弹性空间或底部预留区），
 * 不会挡住任何聊天消息。此时操作栏应当显性呈现，无需用户强行滚到物理列表的最底端。
 */

export type SessionToolbarVisibilityMetrics = {
  scrollHeight: number;
  scrollTop: number;
  clientHeight: number;
  spacer: number;
  safeGap?: number;
};

export function isSessionToolbarBehindElasticSpace(metrics: SessionToolbarVisibilityMetrics): boolean {
  const { scrollHeight, scrollTop, clientHeight, spacer, safeGap = 4 } = metrics;
  if (spacer <= 0) return false;
  const distanceToBottom = scrollHeight - scrollTop - clientHeight;
  // 当视口下沿距物理底部的距离不超过弹性空间 + 安全边距时，
  // 说明真实消息内容底沿完全位于操作栏顶沿之上，操作栏背靠纯弹性空间，未遮挡任何消息。
  return distanceToBottom <= spacer + safeGap;
}
