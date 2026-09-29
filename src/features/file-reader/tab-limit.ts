import { normalizePath } from "./utils";

/**
 * 文件标签数量上限。
 * 达到上限后打开新文件不再新增标签，而是由新标签原地占用当前活跃标签的位置
 * （对齐 Antigravity 编辑器的替换式标签栏，保证标签栏宽度恒定、不会越开越窄）。
 */
export const FILE_READER_MAX_TABS = 4;

/**
 * 决定新标签的落位。
 * @returns -1 表示追加到末尾；>=0 表示应被顶替的标签下标（即当前活跃标签）
 */
export function resolveNewTabSlot(
  tabPaths: string[],
  activePathValue: string,
  maxTabs = FILE_READER_MAX_TABS,
): number {
  if (tabPaths.length < maxTabs) return -1;
  return tabPaths.findIndex((path) => path === activePathValue);
}

/**
 * 恢复会话时把历史标签裁到上限：保留最近打开的后若干项，并确保当前文件仍在列表内。
 * 活跃文件若落在被裁掉的部分，用它替换最旧的一项，避免恢复后找不到当前文件。
 */
export function capRestoredTabPaths(
  paths: string[],
  activeSessionPath: string,
  maxTabs = FILE_READER_MAX_TABS,
): string[] {
  if (paths.length <= maxTabs) return paths;
  const kept = paths.slice(paths.length - maxTabs);
  const normalizedActive = normalizePath(activeSessionPath);
  if (normalizedActive && !kept.some((path) => normalizePath(path) === normalizedActive)) {
    kept[0] = normalizedActive;
  }
  return kept;
}
