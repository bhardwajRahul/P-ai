import { describe, expect, it } from "vitest";
import { sliceNaturalSentencePrefix } from "./text-slicing";

describe("sliceNaturalSentencePrefix", () => {
  it("对短文本且无换行直接返回原文本为 summary", () => {
    const text = "这是一段不到40个字的短文本。";
    const res = sliceNaturalSentencePrefix(text, { availableWidth: 480 });
    expect(res.summary).toBe(text);
    expect(res.remaining).toBe("");
  });

  it("空文本安全返回", () => {
    const res = sliceNaturalSentencePrefix("");
    expect(res.summary).toBe("");
    expect(res.remaining).toBe("");
  });

  it("在宽度范围内的物理换行处自然断开", () => {
    const text = "这是第一行标题内容\n接下来是第二行的详细正文说明。";
    const res = sliceNaturalSentencePrefix(text, { availableWidth: 500 });
    expect(res.summary).toBe("这是第一行标题内容");
    expect(res.remaining).toBe("接下来是第二行的详细正文说明。");
  });

  it("验证用户截图中的中英混排长句：英文按半宽计算，不会在右边还有大片空间时提前换行", () => {
    const text =
      'The user said: "轮椅只守护主会话，追问不需要轮椅" which means "Wheelchair only guards the main session, side questions (追问) don\'t need wheelchair."\nGot it—side views no\'t need the wheelchair logic anymore.';

    // 假设可用宽度为 950px
    const res = sliceNaturalSentencePrefix(text, { availableWidth: 950 });

    // 这一整行在 950px 下完全放得下，包括最后的 wheelchair."，不应该被提前截断
    expect(res.summary.includes("don't need wheelchair.")).toBe(true);
    expect(res.remaining.startsWith("Got it—side views")).toBe(true);
  });

  it("英文长句切断时保证单词完整性，在空格处断开而不会横切单词", () => {
    const text =
      "Antigravity is an intelligent desktop application designed for developers to orchestrate multiple tools";
    // 假设宽度为 240px，大致容纳到 intelligent 或 desktop
    const res = sliceNaturalSentencePrefix(text, { availableWidth: 240 });

    // 切断点必须是完整单词界限，绝不能腰斩单词
    expect(res.summary.endsWith("intelligent") || res.summary.endsWith("desktop")).toBe(true);
    expect(res.remaining.startsWith("desktop") || res.remaining.startsWith("application")).toBe(true);
    expect(`${res.summary} ${res.remaining}`).toBe(text);
  });

  it("超长无空格连续英文 token 兜底在宽度极限处安全截断", () => {
    const text = "A".repeat(100);
    // 100个英文字符 * 6.4px = 640px，限制在 320px（约 50 个字符）
    const res = sliceNaturalSentencePrefix(text, { availableWidth: 320 });

    expect(res.summary.length).toBe(50);
    expect(res.remaining.length).toBe(50);
    expect(res.summary + res.remaining).toBe(text);
  });
});
