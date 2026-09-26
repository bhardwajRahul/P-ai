<template>
  <div class="ecall-sandbox-card" :class="{ 'ecall-sandbox-card-dark': isDark }">
    <div class="ecall-sandbox-toolbar">
      <span class="ecall-sandbox-title">
        <AppWindow class="ecall-sandbox-title-icon" />
        {{ titleText }}
      </span>
      <div class="ecall-sandbox-actions">
        <button
          type="button"
          class="ecall-sandbox-action"
          :title="reloadTitle"
          @click="reload"
        >
          <RotateCw class="ecall-sandbox-action-icon" />
        </button>
        <button
          type="button"
          class="ecall-sandbox-action"
          :class="{ 'ecall-sandbox-action-active': viewMode === 'code' }"
          :title="viewMode === 'code' ? previewTitle : codeTitle"
          @click="toggleViewMode"
        >
          <Code2 v-if="viewMode === 'preview'" class="ecall-sandbox-action-icon" />
          <Eye v-else class="ecall-sandbox-action-icon" />
        </button>
        <button
          type="button"
          class="ecall-sandbox-action"
          :title="popoutTitle"
          @click="popout"
        >
          <ExternalLink class="ecall-sandbox-action-icon" />
        </button>
        <button
          type="button"
          class="ecall-sandbox-action"
          :title="copied ? copiedText : copyTitle"
          @click="copyCode"
        >
          <Check v-if="copied" class="ecall-sandbox-action-icon" />
          <Copy v-else class="ecall-sandbox-action-icon" />
        </button>
        <button
          type="button"
          class="ecall-sandbox-action"
          :title="exportTitle"
          @click="exportHtml"
        >
          <Download class="ecall-sandbox-action-icon" />
        </button>
        <button
          v-if="exitTitle"
          type="button"
          class="ecall-sandbox-action"
          :title="exitTitle"
          @click="emit('exit')"
        >
          <X class="ecall-sandbox-action-icon" />
        </button>
      </div>
    </div>

    <div v-if="viewMode === 'preview'" class="ecall-sandbox-stage" :class="{ 'ecall-sandbox-stage-overflow': heightOverflow }">
      <iframe
        ref="frameRef"
        :key="reloadSeq"
        class="ecall-sandbox-frame"
        :class="{ 'ecall-sandbox-frame-capped': heightOverflow }"
        sandbox="allow-scripts allow-forms allow-modals"
        :srcdoc="srcdoc"
        :style="frameStyle"
        title="interactive-html-sandbox"
      />
      <div v-if="!frameReady && !frameError" class="ecall-sandbox-loading">
        <span class="ecall-sandbox-loading-dot" />
        {{ loadingText }}
      </div>
    </div>
    <pre v-else class="ecall-sandbox-source"><code>{{ code }}</code></pre>

    <div v-if="frameError" class="ecall-sandbox-error">
      <CircleAlert class="ecall-sandbox-error-icon" />
      <span class="ecall-sandbox-error-text">{{ frameError }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { AppWindow, Check, CircleAlert, Code2, Copy, Download, ExternalLink, Eye, RotateCw, X } from "@lucide/vue";
import {
  SANDBOX_MAX_HEIGHT,
  SANDBOX_MESSAGE_TYPE_ERROR,
  SANDBOX_MESSAGE_TYPE_READY,
  SANDBOX_MESSAGE_TYPE_RESIZE,
  SANDBOX_MIN_HEIGHT,
  buildSandboxSrcdoc,
  collectThemeVariables,
} from "../utils/sandbox-html-builder";
import { saveTransportTextFileAs } from "../../../services/tauri-api";

const props = withDefaults(defineProps<{
  code: string;
  isDark?: boolean;
  titleText?: string;
  loadingText?: string;
  reloadTitle?: string;
  codeTitle?: string;
  previewTitle?: string;
  popoutTitle?: string;
  copyTitle?: string;
  copiedText?: string;
  exportTitle?: string;
  /** 非空时显示「退出预览、回到代码」按钮 */
  exitTitle?: string;
}>(), {
  isDark: false,
  titleText: "交互组件",
  loadingText: "正在运行交互组件…",
  reloadTitle: "重新加载",
  codeTitle: "查看源码",
  previewTitle: "返回预览",
  popoutTitle: "新窗口打开",
  copyTitle: "复制代码",
  copiedText: "已复制",
  exportTitle: "导出为 HTML",
});

const emit = defineEmits<{
  (e: "exit"): void;
}>();

const frameRef = ref<HTMLIFrameElement | null>(null);
const viewMode = ref<"preview" | "code">("preview");
const frameHeight = ref(0);
const frameReady = ref(false);
const frameError = ref("");
const heightOverflow = ref(false);
const copied = ref(false);
const reloadSeq = ref(0);
let copyTimer = 0;
let themeObserver: MutationObserver | null = null;
let loadingFallbackTimer = 0;

const themeTokens = computed(() => collectThemeVariables());

const srcdoc = computed(() =>
  buildSandboxSrcdoc(props.code, themeTokens.value, props.isDark),
);

const frameStyle = computed(() => {
  if (heightOverflow.value) return { height: `${SANDBOX_MAX_HEIGHT}px` };
  const height = Math.max(frameHeight.value, SANDBOX_MIN_HEIGHT);
  return { height: `${height}px` };
});

function clampHeight(height: number): number {
  if (!Number.isFinite(height) || height <= 0) return 0;
  return Math.min(Math.max(Math.ceil(height), SANDBOX_MIN_HEIGHT), SANDBOX_MAX_HEIGHT * 4);
}

let shrinkTimer = 0;

function applyFrameHeight(height: number) {
  const next = height > SANDBOX_MAX_HEIGHT ? SANDBOX_MAX_HEIGHT : height;
  heightOverflow.value = height > SANDBOX_MAX_HEIGHT;
  // 变小防抖、变大立即应用、±2px 抖动忽略，避免 ResizeObserver 回环振荡
  if (next === frameHeight.value) return;
  if (next > frameHeight.value) {
    if (shrinkTimer) {
      window.clearTimeout(shrinkTimer);
      shrinkTimer = 0;
    }
    frameHeight.value = next;
    return;
  }
  if (Math.abs(next - frameHeight.value) <= 2) return;
  if (shrinkTimer) window.clearTimeout(shrinkTimer);
  shrinkTimer = window.setTimeout(() => {
    shrinkTimer = 0;
    frameHeight.value = next;
  }, 160);
}

function onMessage(event: MessageEvent) {
  const frame = frameRef.value;
  if (!frame || event.source !== frame.contentWindow) return;
  const data = event.data;
  if (!data || typeof data !== "object") return;
  if (data.type === SANDBOX_MESSAGE_TYPE_RESIZE) {
    const height = clampHeight(Number(data.height));
    if (!height) return;
    applyFrameHeight(height);
    frameReady.value = true;
    return;
  }
  if (data.type === SANDBOX_MESSAGE_TYPE_READY) {
    pushThemeToFrame();
    return;
  }
  if (data.type === SANDBOX_MESSAGE_TYPE_ERROR) {
    frameError.value = String(data.message || "沙箱脚本运行错误");
  }
}

function pushThemeToFrame() {
  const frame = frameRef.value;
  if (!frame || !frame.contentWindow) return;
  try {
    frame.contentWindow.postMessage({
      type: "pai-sandbox:theme",
      tokens: themeTokens.value,
      isDark: props.isDark,
    }, "*");
  } catch {
    // iframe 尚未就绪时忽略
  }
}

function reload() {
  frameError.value = "";
  frameReady.value = false;
  frameHeight.value = 0;
  heightOverflow.value = false;
  reloadSeq.value += 1;
  armLoadingFallback();
}

function toggleViewMode() {
  viewMode.value = viewMode.value === "preview" ? "code" : "preview";
}

function popout() {
  const doc = srcdoc.value;
  if (!doc) return;
  const blob = new Blob([doc], { type: "text/html;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const popup = window.open(url, "_blank", "noopener,noreferrer");
  if (!popup) {
    URL.revokeObjectURL(url);
    frameError.value = "浏览器拦截了弹出窗口";
    return;
  }
  // 延迟回收，保证弹窗完成加载
  window.setTimeout(() => URL.revokeObjectURL(url), 60_000);
}

async function copyCode() {
  try {
    await navigator.clipboard.writeText(props.code || "");
    copied.value = true;
    if (copyTimer) window.clearTimeout(copyTimer);
    copyTimer = window.setTimeout(() => {
      copied.value = false;
      copyTimer = 0;
    }, 1500);
  } catch {
    copied.value = false;
  }
}

async function exportHtml() {
  const doc = srcdoc.value;
  if (!doc) return;
  try {
    await saveTransportTextFileAs(
      `pai-sandbox-${Date.now()}.html`,
      doc,
      [{ name: "HTML", extensions: ["html"] }],
    );
  } catch (error) {
    frameError.value = `导出失败: ${error instanceof Error ? error.message : String(error)}`;
  }
}

function armLoadingFallback() {
  if (loadingFallbackTimer) window.clearTimeout(loadingFallbackTimer);
  loadingFallbackTimer = window.setTimeout(() => {
    // 沙箱内无 Bridge 或脚本异常时保底展示，避免永久 loading
    if (!frameReady.value && !frameError.value) {
      frameReady.value = true;
      frameHeight.value = Math.max(frameHeight.value, 240);
    }
  }, 4000);
}

watch(
  () => [props.isDark, themeTokens.value],
  () => pushThemeToFrame(),
  { deep: true },
);

watch(frameRef, () => {
  frameError.value = "";
  armLoadingFallback();
});

onMounted(() => {
  window.addEventListener("message", onMessage);
  if (typeof MutationObserver === "function" && typeof document !== "undefined") {
    themeObserver = new MutationObserver(() => pushThemeToFrame());
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme", "class", "style"],
    });
  }
  armLoadingFallback();
});

onBeforeUnmount(() => {
  window.removeEventListener("message", onMessage);
  if (themeObserver) {
    themeObserver.disconnect();
    themeObserver = null;
  }
  if (copyTimer) window.clearTimeout(copyTimer);
  if (loadingFallbackTimer) window.clearTimeout(loadingFallbackTimer);
  if (shrinkTimer) window.clearTimeout(shrinkTimer);
});
</script>

<style scoped>
.ecall-sandbox-card {
  position: relative;
  margin: 0.25rem 0;
  max-width: 100%;
}

.ecall-sandbox-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0 0.1rem 0.2rem;
}

.ecall-sandbox-title {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  font-size: var(--app-text-caption-size, 0.72rem);
  color: color-mix(in srgb, currentColor 70%, transparent);
  min-width: 0;
}

.ecall-sandbox-title-icon {
  width: 0.85rem;
  height: 0.85rem;
  flex-shrink: 0;
}

.ecall-sandbox-actions {
  display: flex;
  align-items: center;
  gap: 0.15rem;
  flex-shrink: 0;
}

.ecall-sandbox-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.45rem;
  height: 1.45rem;
  border-radius: 0.3rem;
  color: color-mix(in srgb, currentColor 62%, transparent);
  transition: background-color 0.12s ease, color 0.12s ease;
}

.ecall-sandbox-action:hover {
  background: color-mix(in srgb, currentColor 10%, transparent);
  color: currentColor;
}

.ecall-sandbox-action-active {
  color: var(--color-primary, currentColor);
  background: color-mix(in srgb, var(--color-primary, currentColor) 14%, transparent);
}

.ecall-sandbox-action-icon {
  width: 0.85rem;
  height: 0.85rem;
}

.ecall-sandbox-stage {
  position: relative;
  width: 100%;
}

.ecall-sandbox-stage-overflow {
  overflow-y: auto;
}

.ecall-sandbox-frame {
  display: block;
  width: 100%;
  min-height: 120px;
  border: 0;
  background: transparent;
}

.ecall-sandbox-loading {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.45rem;
  font-size: var(--app-text-xs-size, 0.78rem);
  color: color-mix(in srgb, currentColor 55%, transparent);
  background: color-mix(in srgb, currentColor 3%, transparent);
  pointer-events: none;
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

.ecall-sandbox-source {
  margin: 0;
  max-height: 420px;
  overflow: auto;
  padding: 0.75rem;
  border: 1px solid color-mix(in srgb, currentColor 14%, transparent);
  border-radius: 0.5rem;
  background: color-mix(in srgb, currentColor 5%, transparent);
  font-family: var(--app-code-font-family, monospace);
  font-size: var(--app-text-xs-size, 0.78rem);
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-all;
  color: color-mix(in srgb, currentColor 85%, transparent);
}

.ecall-sandbox-error {
  display: flex;
  align-items: flex-start;
  gap: 0.4rem;
  padding: 0.45rem 0.6rem;
  margin-top: 0.25rem;
  border: 1px solid color-mix(in srgb, var(--color-error, #f87272) 40%, transparent);
  border-radius: 0.4rem;
  background: color-mix(in srgb, var(--color-error, #f87272) 10%, transparent);
  font-size: var(--app-text-caption-size, 0.72rem);
  color: var(--color-error, #f87272);
}

.ecall-sandbox-error-icon {
  width: 0.85rem;
  height: 0.85rem;
  flex-shrink: 0;
  margin-top: 0.05rem;
}

.ecall-sandbox-error-text {
  min-width: 0;
  word-break: break-all;
}
</style>
