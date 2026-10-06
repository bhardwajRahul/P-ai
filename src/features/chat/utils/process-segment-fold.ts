/** 过程段折叠意图。auto 走默认；expanded / collapsed 是用户点过之后的选择。 */
export type ProcessSegmentFoldIntent = "auto" | "expanded" | "collapsed";

export function shouldFoldProcessSegments(input: {
  foldEnabled: boolean;
  pieceCount: number;
  bubbleBackground: boolean;
  streaming: boolean;
  intent: ProcessSegmentFoldIntent;
}): boolean {
  if (!input.foldEnabled || input.pieceCount <= 1) return false;
  if (input.intent === "expanded") return false;
  if (input.intent === "collapsed") return true;
  // 无气泡流式先全部摊开，输出结束再收成只留最后一段。气泡模式始终默认折叠。
  if (!input.bubbleBackground && input.streaming) return false;
  return true;
}
