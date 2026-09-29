import { describe, expect, it } from "vitest";
import { capRestoredTabPaths, FILE_READER_MAX_TABS, resolveNewTabSlot } from "./tab-limit";

describe("resolveNewTabSlot", () => {
  it("未达上限时追加到末尾", () => {
    expect(resolveNewTabSlot([], "/a")).toBe(-1);
    expect(resolveNewTabSlot(["/a", "/b", "/c"], "/b")).toBe(-1);
  });

  it("达上限时顶替当前活跃标签的位置", () => {
    const paths = ["/a", "/b", "/c", "/d"];
    expect(paths.length).toBe(FILE_READER_MAX_TABS);
    // 活跃的是第三项 /c，新文件应顶到它的下标
    expect(resolveNewTabSlot(paths, "/c")).toBe(2);
  });

  it("活跃文件不在列表中时按追加处理，避免误删", () => {
    expect(resolveNewTabSlot(["/a", "/b", "/c", "/d"], "/missing")).toBe(-1);
  });

  it("上限可覆盖", () => {
    expect(resolveNewTabSlot(["/a"], "/a", 1)).toBe(0);
  });
});

describe("capRestoredTabPaths", () => {
  it("不超过上限时原样返回", () => {
    expect(capRestoredTabPaths(["/a", "/b"], "/a")).toEqual(["/a", "/b"]);
  });

  it("超过上限时保留最近打开的后若干项", () => {
    expect(capRestoredTabPaths(["/a", "/b", "/c", "/d", "/e"], "/e"))
      .toEqual(["/b", "/c", "/d", "/e"]);
  });

  it("活跃文件被裁掉时替换最旧的一项，保证仍可见", () => {
    expect(capRestoredTabPaths(["/a", "/b", "/c", "/d", "/e"], "/a"))
      .toEqual(["/a", "/c", "/d", "/e"]);
  });

  it("活跃路径为空时不额外插入", () => {
    expect(capRestoredTabPaths(["/a", "/b", "/c", "/d", "/e"], ""))
      .toEqual(["/b", "/c", "/d", "/e"]);
  });
});
