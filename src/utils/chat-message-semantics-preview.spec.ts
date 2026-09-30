import { describe, expect, it } from "vitest";
import { stripPreviewMarkdown } from "./chat-message-semantics";

describe("stripPreviewMarkdown", () => {
  it("去掉行内代码的反引号，保留文字", () => {
    expect(stripPreviewMarkdown("玛拉妮改好并部署（880）: `e, w`")).toBe("玛拉妮改好并部署（880）: e, w");
  });

  it("去掉加粗、斜体、删除线标记", () => {
    expect(stripPreviewMarkdown("**重点**和*斜体*以及~~删除~~")).toBe("重点和斜体以及删除");
    expect(stripPreviewMarkdown("__加粗__ 与 `代码`")).toBe("加粗 与 代码");
  });

  it("去掉行首标题、引用与列表标记", () => {
    expect(stripPreviewMarkdown("## 小标题")).toBe("小标题");
    expect(stripPreviewMarkdown("> 引用内容")).toBe("引用内容");
    expect(stripPreviewMarkdown("- 第一项")).toBe("第一项");
    expect(stripPreviewMarkdown("1. 第一步")).toBe("第一步");
  });

  it("链接与图片只保留可见文字", () => {
    expect(stripPreviewMarkdown("见 [文档](https://example.com/a)")).toBe("见 文档");
    expect(stripPreviewMarkdown("![截图](https://example.com/a.png)已更新")).toBe("截图已更新");
  });

  it("围栏代码块只去围栏，保留内部文本", () => {
    expect(stripPreviewMarkdown("```ts\nconst a = 1;\n```")).toBe("const a = 1;");
  });

  it("多行文本压成单行", () => {
    expect(stripPreviewMarkdown("第一行\n\n第二行")).toBe("第一行 第二行");
  });

  it("保留 snake_case 标识符（下划线不参与剥离）", () => {
    expect(stripPreviewMarkdown("user_persona 与 agent_id")).toBe("user_persona 与 agent_id");
  });

  it("普通文本原样保留", () => {
    expect(stripPreviewMarkdown("分五笔提交完成，工作区干净了。")).toBe("分五笔提交完成，工作区干净了。");
  });
});
