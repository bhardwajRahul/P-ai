import { describe, expect, it } from "vitest";
import {
  buildSandboxSrcdoc,
  isInteractiveHtmlLang,
  isPlainHtmlLang,
} from "./sandbox-html-builder";

describe("isInteractiveHtmlLang", () => {
  it("识别 html:interactive / html:preview / html:widget", () => {
    expect(isInteractiveHtmlLang("html:interactive")).toBe(true);
    expect(isInteractiveHtmlLang("HTML:Preview")).toBe(true);
    expect(isInteractiveHtmlLang("html:widget")).toBe(true);
  });

  it("不识别普通 html 或其他语言", () => {
    expect(isInteractiveHtmlLang("html")).toBe(false);
    expect(isInteractiveHtmlLang("svg")).toBe(false);
    expect(isInteractiveHtmlLang("")).toBe(false);
    expect(isInteractiveHtmlLang("htmlx:interactive")).toBe(false);
    expect(isInteractiveHtmlLang("html:other")).toBe(false);
  });
});

describe("isPlainHtmlLang", () => {
  it("识别 html / htm", () => {
    expect(isPlainHtmlLang("html")).toBe(true);
    expect(isPlainHtmlLang("HTM")).toBe(true);
  });

  it("不识别 html:interactive", () => {
    expect(isPlainHtmlLang("html:interactive")).toBe(false);
  });
});

describe("buildSandboxSrcdoc", () => {
  const tokens = { "--color-base-100": "#ffffff", "--color-primary": "#3366ff" };

  it("为裸片段补齐文档骨架", () => {
    const out = buildSandboxSrcdoc("<div>hi</div>", tokens, false);
    expect(out).toContain("<!DOCTYPE html>");
    expect(out).toContain("<div>hi</div>");
  });

  it("注入 CSP meta", () => {
    const out = buildSandboxSrcdoc(
      '<!DOCTYPE html><html><head></head><body><p>x</p></body></html>',
      tokens,
      false,
    );
    expect(out).toContain("Content-Security-Policy");
    expect(out).toContain("default-src 'none'");
  });

  it("替换已有 CSP meta", () => {
    const out = buildSandboxSrcdoc(
      '<html><head><meta http-equiv="Content-Security-Policy" content="default-src *"></head><body>x</body></html>',
      tokens,
      false,
    );
    expect(out).toContain("default-src 'none'");
    expect(out).not.toContain("default-src *");
  });

  it("注入主题变量与明暗模式", () => {
    const out = buildSandboxSrcdoc("<p>x</p>", tokens, true);
    expect(out).toContain("--color-base-100: #ffffff");
    expect(out).toContain("--color-primary: #3366ff");
    expect(out).toContain('color-scheme: dark');
  });

  it("注入 Bridge 脚本", () => {
    const out = buildSandboxSrcdoc("<p>x</p>", tokens, false);
    expect(out).toContain("pai-sandbox:resize");
    expect(out).toContain("data-pai-bridge");
  });

  it("空输入返回空串", () => {
    expect(buildSandboxSrcdoc("   ", tokens, false)).toBe("");
  });
});
