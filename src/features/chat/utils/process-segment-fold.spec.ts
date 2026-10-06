import { describe, expect, it } from "vitest";
import { shouldFoldProcessSegments } from "./process-segment-fold";

const base = {
  foldEnabled: true,
  pieceCount: 3,
  intent: "auto" as const,
};

describe("process segment fold", () => {
  it("气泡模式流式中也默认折叠", () => {
    expect(shouldFoldProcessSegments({
      ...base,
      bubbleBackground: true,
      streaming: true,
    })).toBe(true);
  });

  it("无气泡流式中默认摊开，结束后再折叠", () => {
    expect(shouldFoldProcessSegments({
      ...base,
      bubbleBackground: false,
      streaming: true,
    })).toBe(false);
    expect(shouldFoldProcessSegments({
      ...base,
      bubbleBackground: false,
      streaming: false,
    })).toBe(true);
  });

  it("手动展开后，流式结束也不再收起", () => {
    expect(shouldFoldProcessSegments({
      ...base,
      bubbleBackground: false,
      streaming: false,
      intent: "expanded",
    })).toBe(false);
    expect(shouldFoldProcessSegments({
      ...base,
      bubbleBackground: true,
      streaming: false,
      intent: "expanded",
    })).toBe(false);
  });

  it("手动收起后，无气泡流式中也保持折叠", () => {
    expect(shouldFoldProcessSegments({
      ...base,
      bubbleBackground: false,
      streaming: true,
      intent: "collapsed",
    })).toBe(true);
  });

  it("开关关闭或只有一段时不折叠", () => {
    expect(shouldFoldProcessSegments({
      ...base,
      foldEnabled: false,
      bubbleBackground: true,
      streaming: false,
    })).toBe(false);
    expect(shouldFoldProcessSegments({
      ...base,
      pieceCount: 1,
      bubbleBackground: false,
      streaming: false,
    })).toBe(false);
  });
});
