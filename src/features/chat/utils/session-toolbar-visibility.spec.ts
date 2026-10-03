// @vitest-environment node
import { describe, expect, it } from "vitest";
import { isSessionToolbarBehindElasticSpace } from "./session-toolbar-visibility";

describe("isSessionToolbarBehindElasticSpace", () => {
  it("无弹性留白（spacer <= 0）时始终返回 false", () => {
    expect(
      isSessionToolbarBehindElasticSpace({
        scrollHeight: 1000,
        scrollTop: 100,
        clientHeight: 800,
        spacer: 0,
      }),
    ).toBe(false);
  });

  it("当距底距离在弹性空间以内时返回 true（操作栏背后全为留白，未挡住消息）", () => {
    // 视口下沿 800，物理底部 1200，距底 400px；弹性留白有 500px
    // 真实消息在物理底部 - 500 = 700px 处已结束，位于视口下沿 800 之前
    expect(
      isSessionToolbarBehindElasticSpace({
        scrollHeight: 1200,
        scrollTop: 0,
        clientHeight: 800,
        spacer: 500,
      }),
    ).toBe(true);
  });

  it("当向上滚动、距底距离超过弹性空间时返回 false（真实消息落入操作栏背后）", () => {
    // 视口下沿 400，物理底部 1200，距底 800px；弹性留白仅 500px
    // 真实消息底沿在 700px，当前视口在 0~400，但在滚动范围中消息延伸至 700，操作栏若在底部会遮挡消息
    expect(
      isSessionToolbarBehindElasticSpace({
        scrollHeight: 1200,
        scrollTop: 0,
        clientHeight: 400,
        spacer: 500,
      }),
    ).toBe(false);
  });

  it("临界容差安全边距校验", () => {
    // 距底 504px，spacer 500px，safeGap 4px -> 恰好满足 504 <= 504
    expect(
      isSessionToolbarBehindElasticSpace({
        scrollHeight: 1504,
        scrollTop: 0,
        clientHeight: 1000,
        spacer: 500,
        safeGap: 4,
      }),
    ).toBe(true);

    // 距底 505px，超过 safeGap -> 返回 false
    expect(
      isSessionToolbarBehindElasticSpace({
        scrollHeight: 1505,
        scrollTop: 0,
        clientHeight: 1000,
        spacer: 500,
        safeGap: 4,
      }),
    ).toBe(false);
  });
});
