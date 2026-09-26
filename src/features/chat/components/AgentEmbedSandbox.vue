<template>
  <InteractiveHtmlSandbox
    v-if="loadedCode"
    :code="loadedCode"
    :is-dark="isDark"
    :title-text="displayName"
  />
  <div v-else-if="loadError" class="ecall-agent-embed-fallback">
    <div class="ecall-agent-embed-fallback-text">
      无法加载嵌入文件：{{ loadError }}
    </div>
    <pre class="ecall-agent-embed-fallback-src"><code>{{ rawSrc }}</code></pre>
  </div>
  <div v-else class="ecall-agent-embed-loading">
    <span class="ecall-sandbox-loading-dot" />
    {{ loadingText }}
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import InteractiveHtmlSandbox from "./InteractiveHtmlSandbox.vue";
import { invokeTauri } from "../../../services/tauri-api";
import { isAbsoluteLocalPath, normalizeLocalLinkHref } from "../utils/local-link";
import type { FileReaderFileBlockPayload, FileReaderFilePayload } from "../../file-reader/types";

const EMBED_MAX_CHARS = 512 * 1024;
// 与后端 FILE_READER_BLOCK_LINE_COUNT 对齐
const FILE_BLOCK_LINES = 120;

const props = withDefaults(defineProps<{
  src: string;
  isDark?: boolean;
  loadingText?: string;
}>(), {
  isDark: false,
  loadingText: "正在加载嵌入组件…",
});

const loadedCode = ref("");
const loadError = ref("");
let loadSeq = 0;

const rawSrc = computed(() => String(props.src || "").trim());

const displayName = computed(() => {
  const normalized = rawSrc.value.replace(/\\/g, "/");
  const name = normalized.split("/").filter(Boolean).pop() || "嵌入组件";
  return name.replace(/\.(html?|svg)$/i, "") || name;
});

async function load() {
  const seq = ++loadSeq;
  loadedCode.value = "";
  loadError.value = "";

  const src = rawSrc.value;
  if (!src) {
    loadError.value = "缺少 src";
    return;
  }
  if (!/\.(html?|svg)$/i.test(src)) {
    loadError.value = "仅支持嵌入 .html / .svg 文件";
    return;
  }
  const path = normalizeLocalLinkHref(src);
  if (!path) {
    loadError.value = "路径无法解析";
    return;
  }
  if (!isAbsoluteLocalPath(path)) {
    loadError.value = "src 必须是绝对路径";
    return;
  }
  try {
    const meta = await invokeTauri<FileReaderFilePayload>("fileReader.readFile", { path });
    if (seq !== loadSeq) return;
    let content = String(meta?.content ?? "");
    if (meta?.virtualized) {
      // code 类文件（含 .html）走虚拟化：content 为空，按块拼接全文
      const totalLines = Math.max(0, Number(meta.totalLines) || 0);
      const parts: string[] = [];
      for (let start = 1; start <= totalLines; start += FILE_BLOCK_LINES) {
        const block = await invokeTauri<FileReaderFileBlockPayload>("fileReader.readFileBlock", {
          path,
          startLine: start,
          lineCount: FILE_BLOCK_LINES,
        });
        if (seq !== loadSeq) return;
        parts.push(String(block?.content ?? ""));
        if (contentLength(parts) > EMBED_MAX_CHARS) break;
      }
      content = parts.join("");
    }
    if (!content.trim()) {
      loadError.value = "文件为空或不存在";
      return;
    }
    loadedCode.value = content.length > EMBED_MAX_CHARS ? content.slice(0, EMBED_MAX_CHARS) : content;
  } catch (error) {
    if (seq !== loadSeq) return;
    loadError.value = error instanceof Error ? error.message : String(error);
  }
}

function contentLength(parts: string[]): number {
  let total = 0;
  for (const part of parts) total += part.length;
  return total;
}

watch(
  () => props.src,
  () => void load(),
  { immediate: true },
);
</script>

<style scoped>
.ecall-agent-embed-loading {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  margin: 0.25rem 0;
  padding: 0.75rem 0.1rem;
  font-size: var(--app-text-xs-size, 0.78rem);
  color: color-mix(in srgb, currentColor 55%, transparent);
}

.ecall-sandbox-loading-dot {
  width: 0.5rem;
  height: 0.5rem;
  border-radius: 999px;
  background: var(--color-primary, currentColor);
  opacity: 0.7;
  animation: ecall-sandbox-pulse 1.1s ease-in-out infinite;
}

@keyframes ecall-sandbox-pulse {
  0%, 100% { transform: scale(0.7); opacity: 0.4; }
  50% { transform: scale(1); opacity: 0.9; }
}

.ecall-agent-embed-fallback {
  margin: 0.25rem 0;
  padding: 0.55rem 0.7rem;
  border: 1px dashed color-mix(in srgb, currentColor 20%, transparent);
  border-radius: 0.5rem;
}

.ecall-agent-embed-fallback-text {
  font-size: var(--app-text-xs-size, 0.78rem);
  color: color-mix(in srgb, currentColor 65%, transparent);
}

.ecall-agent-embed-fallback-src {
  margin: 0.35rem 0 0;
  font-family: var(--app-code-font-family, monospace);
  font-size: var(--app-text-caption-size, 0.72rem);
  color: color-mix(in srgb, currentColor 55%, transparent);
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
