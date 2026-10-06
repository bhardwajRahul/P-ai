import { createHighlighter } from "shiki";
import { describe, expect, it } from "vitest";
import { canOpenInFileReader } from "../chat/utils/chat-render";
import { ceaLanguage } from "./cea-language";
import { fileKindFromPath, resolveShikiLanguage } from "./utils";

const SAMPLE = `
[ENABLE]
define(newmem,09380000)
newmem: // allocated
{
hidden:
mov eax, 1
}
start:
    dd newColor
    dd ffb0a800
    call 004DC620
    mov edx, eax // pointer
    mov [edx+e0], eax
    push #666
`.trim();

describe("cea language", () => {
  it("把 cea 当成可打开的代码，并使用自带语法", () => {
    expect(canOpenInFileReader("D:/mod/剧本标题控件上色.cea")).toBe(true);
    expect(fileKindFromPath("D:/mod/剧本标题控件上色.cea")).toBe("code");
    expect(resolveShikiLanguage("cea")).toBe("cea");
  });

  it("注释、标签、指令、寄存器和数字分开着色", async () => {
    const highlighter = await createHighlighter({
      langs: [ceaLanguage],
      themes: ["github-light"],
    });
    const { tokens } = highlighter.codeToTokens(SAMPLE, {
      lang: "cea",
      theme: "github-light",
      includeExplanation: true,
    });
    expect(scopeContaining(tokens, "// allocated")).toContain("comment.line.double-slash.cea");
    expect(scopeContaining(tokens, "hidden")).toContain("comment.block.cea");
    expect(exactScopes(tokens, "hidden").some((scope) => scope.includes("entity.name.label.cea"))).toBe(false);
    expect(exactScopes(tokens, "[ENABLE]").some((scope) => scope.includes("keyword.control.section.cea"))).toBe(true);
    expect(exactScopes(tokens, "define").some((scope) => scope.includes("keyword.control.directive.cea"))).toBe(true);
    expect(exactScopes(tokens, "newmem").some((scope) => scope.includes("entity.name.label.cea"))).toBe(true);
    expect(exactScopes(tokens, "09380000").some((scope) => scope.includes("constant.numeric.hex.cea"))).toBe(true);
    expect(exactScopes(tokens, "ffb0a800").some((scope) => scope.includes("constant.numeric.hex.cea"))).toBe(true);
    expect(exactScopes(tokens, "newColor").some((scope) => scope.includes("constant.numeric.hex.cea"))).toBe(false);
    expect(exactScopes(tokens, "call").some((scope) => scope.includes("keyword.operator.instruction.cea"))).toBe(true);
    expect(exactScopes(tokens, "eax").some((scope) => scope.includes("variable.language.register.cea"))).toBe(true);
    expect(exactScopes(tokens, "dd").some((scope) => scope.includes("storage.type.data.cea"))).toBe(true);
    expect(exactScopes(tokens, "#666").some((scope) => scope.includes("constant.numeric.decimal.cea"))).toBe(true);
  });
});

type HighlightToken = {
  content: string;
  explanation?: Array<{ scopes: Array<{ scopeName: string }> }>;
};

function tokenScope(token: HighlightToken) {
  return (token.explanation || [])
    .flatMap((item) => item.scopes.map((scope) => scope.scopeName))
    .join(" ");
}

function exactScopes(lines: HighlightToken[][], text: string) {
  return lines.flatMap((line) => line.filter((token) => token.content === text).map(tokenScope));
}

function scopeContaining(lines: HighlightToken[][], text: string) {
  for (const line of lines) {
    for (const token of line) {
      if (token.content.includes(text)) return tokenScope(token);
    }
  }
  return "";
}
