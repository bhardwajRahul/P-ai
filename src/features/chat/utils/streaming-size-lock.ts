/** 流式气泡只增不减的尺寸锁。锁只属于当前正在输出的那一段。 */

export type StreamingSizeLockState = {
  pieceIndex: number;
  height: number;
  width: number;
};

export function createStreamingSizeLock(): StreamingSizeLockState {
  return { pieceIndex: -1, height: 0, width: 0 };
}

/** 换段后的那一帧旧高度不能套上新段，否则折叠收起的上一段会把新气泡撑出空白。 */
export function streamingSizeLockStyle(
  state: StreamingSizeLockState,
  pieceIndex: number,
): { minHeight?: string; minWidth?: string } | undefined {
  if (pieceIndex < 0 || state.pieceIndex !== pieceIndex) return undefined;
  const styles: { minHeight?: string; minWidth?: string } = {};
  if (state.height > 0) styles.minHeight = `${state.height}px`;
  if (state.width > 0) styles.minWidth = `min(${state.width}px, 100%)`;
  return styles.minHeight || styles.minWidth ? styles : undefined;
}

/**
 * 同一段只升不降。pieceIndex 变了说明上一段已经收起或被工具切开，
 * 旧高度作废，改记新段自己的尺寸。
 */
export function observeStreamingSize(
  state: StreamingSizeLockState,
  pieceIndex: number,
  height: number,
  width: number,
  parentWidth: number,
): StreamingSizeLockState {
  const nextHeight = Math.max(0, height);
  const cappedWidth = parentWidth > 0 ? Math.min(Math.max(0, width), parentWidth) : Math.max(0, width);
  if (state.pieceIndex !== pieceIndex) {
    return { pieceIndex, height: nextHeight, width: cappedWidth };
  }
  const widthLocked = parentWidth > 0 && state.width > parentWidth ? parentWidth : state.width;
  const heightLocked = Math.max(state.height, nextHeight);
  const widthNext = Math.max(widthLocked, cappedWidth);
  if (heightLocked === state.height && widthNext === state.width) return state;
  return { pieceIndex, height: heightLocked, width: widthNext };
}
