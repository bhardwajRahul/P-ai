import { describe, expect, it } from "vitest";
import {
  createStreamingSizeLock,
  observeStreamingSize,
  streamingSizeLockStyle,
} from "./streaming-size-lock";

describe("streaming size lock", () => {
  it("同一段只升不降", () => {
    const grown = observeStreamingSize(createStreamingSizeLock(), 0, 80, 120, 400);
    const shrunk = observeStreamingSize(grown, 0, 40, 80, 400);
    expect(shrunk).toBe(grown);
    expect(streamingSizeLockStyle(shrunk, 0)?.minHeight).toBe("80px");
  });

  it("换段后不沿用上一段高度", () => {
    const previous = observeStreamingSize(createStreamingSizeLock(), 0, 200, 320, 400);
    const next = observeStreamingSize(previous, 1, 48, 90, 400);
    expect(next.height).toBe(48);
    expect(next.width).toBe(90);
    expect(streamingSizeLockStyle(previous, 1)).toBeUndefined();
    expect(streamingSizeLockStyle(next, 1)?.minHeight).toBe("48px");
  });

  it("窗口变窄时宽度锁跟着收，不超过父容器", () => {
    const wide = observeStreamingSize(createStreamingSizeLock(), 0, 40, 320, 400);
    const narrowed = observeStreamingSize(wide, 0, 40, 320, 180);
    expect(narrowed.width).toBe(180);
  });
});
