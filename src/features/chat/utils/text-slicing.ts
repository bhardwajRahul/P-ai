/**
 * 文本断句与智能首行切分工具
 * 根据容器物理像素宽度自适应填充单行，解决中英文视觉宽度不一致导致的“右侧大片空白”问题，
 * 同时保证英文单词完整性与剩余文本零丢失。
 */

export interface NaturalSentenceSplitOptions {
  /**
   * 容器可用物理像素宽度（单位 px，默认约 480px）
   */
  availableWidth?: number;
  /**
   * 字符基准数兼容模式（若只传入 targetLength，自动按汉字 12.2px 换算为像素宽度）
   */
  targetLength?: number;
}

export interface SplitResult {
  /**
   * 提取的首行内容（填满单行，去除末尾多余空格）
   */
  summary: string;
  /**
   * 剩余的所有内容（无损衔接，清理切分点紧邻的单个换行或前导空格）
   */
  remaining: string;
}

function isAsciiLetterOrDigit(char: string | undefined): boolean {
  if (!char) return false;
  const code = char.charCodeAt(0);
  return (
    (code >= 65 && code <= 90) || // A-Z
    (code >= 97 && code <= 122) || // a-z
    (code >= 48 && code <= 57) // 0-9
  );
}

/**
 * 获取单个字符在 text-xs (12px) 样式下的估算物理渲染像素宽度：
 * - ASCII 半角字符（英文、数字、空格、半角标点）：约 6.4px
 * - 中文汉字、全角标点、Emoji、CJK 字符：约 12.2px
 */
export function getCharRenderPixelWidth(char: string): number {
  if (!char) return 0;
  const code = char.charCodeAt(0);
  // ASCII 可打印字符与常规空格
  if (code >= 0x20 && code <= 0x7e) {
    return 6.4;
  }
  // 全角/中文等
  return 12.2;
}

function cleanSplitResult(text: string, splitIndex: number): SplitResult {
  const summary = text.slice(0, splitIndex).trimEnd();
  let remaining = text.slice(splitIndex);

  // 清除紧随断开处的首个换行或前导空格，保证正文区不出现无意义空行或前导缩进
  if (remaining.startsWith("\r\n")) {
    remaining = remaining.slice(2);
  } else if (remaining.startsWith("\n") || remaining.startsWith("\r") || remaining.startsWith(" ")) {
    remaining = remaining.slice(1);
  }

  return { summary, remaining };
}

/**
 * 根据容器实际物理可用像素宽度切分首行：
 * 1. 遇到物理换行符（在宽度容纳范围内）：以物理换行作为首行终止。
 * 2. 物理宽度限制：按字符真实渲染宽度（半角 ~6.4px，全角 ~12.2px）累加，直到触达容器右边界。
 * 3. 英文单词保护：若切断点恰好落在英文单词内部，向左回退到单词开头的空格，绝不劈开单词。
 * 4. 剩余文本无损传递给下文。
 */
export function sliceNaturalSentencePrefix(
  rawText: string,
  options: NaturalSentenceSplitOptions = {}
): SplitResult {
  const text = String(rawText || "");
  if (!text) {
    return { summary: "", remaining: "" };
  }

  // 计算目标像素宽度：优先使用 availableWidth；若只传了 targetLength，换算为像素
  let maxPixelWidth = options.availableWidth;
  if (maxPixelWidth === undefined || maxPixelWidth <= 0) {
    if (options.targetLength !== undefined && options.targetLength > 0) {
      maxPixelWidth = options.targetLength * 12.2;
    } else {
      maxPixelWidth = 480;
    }
  }

  // 1. 逐字符累加物理渲染宽度
  let splitIndex = text.length;
  let currentWidth = 0;

  for (let i = 0; i < text.length; i++) {
    const char = text[i];

    // 遇到物理换行符
    if (char === "\n" || char === "\r") {
      // 只有换行前有内容才切断
      if (i > 0) {
        splitIndex = i;
        return cleanSplitResult(text, splitIndex);
      }
    }

    const charWidth = getCharRenderPixelWidth(char);
    if (currentWidth + charWidth > maxPixelWidth) {
      splitIndex = i;
      break;
    }
    currentWidth += charWidth;
  }

  // 2. 英文单词完整性保护
  if (splitIndex < text.length) {
    const prevChar = text[splitIndex - 1];
    const currChar = text[splitIndex];

    // 若切断点恰好在英文单词中间（两端都是字母数字）
    if (isAsciiLetterOrDigit(prevChar) && isAsciiLetterOrDigit(currChar)) {
      const lastSpace = text.lastIndexOf(" ", splitIndex);
      // 回退到单词开头的空格，避免劈开单词（限制回退范围不超过 24 字符）
      if (lastSpace >= 0 && splitIndex - lastSpace <= 24) {
        splitIndex = lastSpace;
      }
    }
  }

  return cleanSplitResult(text, splitIndex);
}
