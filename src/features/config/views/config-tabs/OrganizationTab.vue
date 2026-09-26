<template>
  <SettingsStickyLayout>
    <template #header>
      <div class="flex w-full flex-col gap-3">
        <div class="flex items-center justify-between">
          <span class="text-sm font-semibold">{{ t("config.organization.title") }}</span>
          <div class="flex items-center gap-1">
            <button
              class="btn btn-sm btn-square btn-ghost"
              type="button"
              :title="t('common.reset')"
              :disabled="!relationDirty || saving"
              @click="restoreDraftsFromSaved"
            >
              <RotateCcw class="h-3.5 w-3.5" />
            </button>
            <button
              class="btn btn-sm btn-square"
              type="button"
              :class="relationDirty ? 'btn-primary' : 'btn-ghost'"
              :disabled="!selectedPersona || !!relationValidationMessage || !relationDirty || saving"
              :title="saving ? t('config.api.saving') : relationDirty ? t('common.save') : t('status.configSaved')"
              @click="savePersonaRelations"
            >
              <Save v-if="!saving" class="h-3.5 w-3.5" />
              <span v-else class="loading loading-spinner loading-sm"></span>
            </button>
          </div>
        </div>
        <div class="text-sm opacity-60">{{ t("config.organization.hint") }}</div>
      </div>
    </template>

    <div class="flex min-h-full flex-col gap-3">
      <!-- 直属下级快速设置：点选节点后在此勾选/取消该节点的下级 -->
      <div class="overflow-hidden rounded-box border border-base-300 bg-base-100">
        <div v-if="relationValidationMessage" class="border-b border-warning/30 bg-warning/10 px-4 py-3 text-sm text-warning-content">
          {{ relationValidationMessage }}
        </div>

        <div class="px-4 py-4">
          <div class="mb-3 flex items-center justify-between gap-3">
            <div class="text-sm font-medium">
              {{ selectedPersona ? t("config.organization.childrenOf", { name: selectedPersonaDisplayName }) : t("config.organization.directChildren") }}
            </div>
            <div class="text-xs opacity-50">{{ selectedChildIds.length }} / {{ candidatePersonas.length }}</div>
          </div>
          <div class="mb-3 text-sm opacity-60">
            {{ t("config.organization.directChildrenHint") }}
          </div>

          <div v-if="!selectedPersona" class="text-sm opacity-60">
            {{ t("config.organization.selectHint") }}
          </div>
          <div v-else-if="candidatePersonas.length === 0" class="text-sm opacity-60">
            {{ t("config.organization.noCandidateChildren") }}
          </div>
          <div v-else class="flex flex-wrap gap-2">
            <button
              v-for="persona in candidatePersonas"
              :key="persona.id"
              type="button"
              class="inline-flex h-9 max-w-full items-center gap-2 rounded-lg border px-3 text-left transition"
              :class="selectedChildIds.includes(persona.id)
                ? 'border-primary/50 bg-primary/10 text-base-content shadow-sm'
                : 'border-base-content/10 bg-base-100 text-base-content hover:border-base-content/20'"
              @click="toggleChildPersona(persona.id)"
            >
              <span
                class="flex h-4 w-4 shrink-0 items-center justify-center rounded border transition"
                :class="selectedChildIds.includes(persona.id)
                  ? 'border-primary bg-primary text-primary-content'
                  : 'border-base-content/20 bg-base-200 text-transparent'"
              >
                <Check class="h-3 w-3" />
              </span>
              <span
                class="flex h-6 w-6 shrink-0 items-center justify-center overflow-hidden rounded-full bg-base-200 text-[10px] font-semibold text-base-content/70"
              >
                <img
                  v-if="avatarOf(persona.id)"
                  :src="avatarOf(persona.id)"
                  :alt="persona.name"
                  class="h-full w-full object-cover"
                />
                <span v-else>{{ initialOf(persona.name) }}</span>
              </span>
              <span class="truncate text-sm font-medium">{{ persona.name }}</span>
            </button>
          </div>
        </div>
      </div>

      <Teleport to="body" :disabled="!isFlowFullscreen">
        <div
          class="organization-tree-graph-host"
          :class="isFlowFullscreen
            ? 'fixed inset-0 z-[120] flex items-center justify-center bg-base-300/45 p-4 backdrop-blur-sm'
            : 'flex min-h-[560px] flex-1'"
          @click.self="closeFlowFullscreen"
        >
          <div
            :class="isFlowFullscreen
              ? 'flex h-[94vh] w-[94vw] max-w-none flex-col overflow-hidden rounded-box border border-base-300 bg-base-100 shadow-2xl'
              : 'organization-tree-graph-card flex min-h-[560px] flex-1 flex-col overflow-hidden rounded-box border border-base-300 bg-base-100'"
          >
            <div v-if="isFlowFullscreen" class="flex items-center justify-between border-b border-base-300 px-4 py-3">
              <div class="text-sm font-medium">{{ t("config.organization.title") }}</div>
              <button
                class="btn btn-sm btn-square bg-base-200"
                type="button"
                :title="t('config.organization.exitFullscreen')"
                @click="closeFlowFullscreen"
              >
                <Minimize2 class="h-3.5 w-3.5" />
              </button>
            </div>

            <VueFlow
              v-model:nodes="flowNodes"
              v-model:edges="flowEdges"
              class="organization-tree-flow min-h-0 w-full flex-1"
              :min-zoom="0.35"
              :max-zoom="1.8"
              :nodes-draggable="true"
              :nodes-connectable="false"
              :elements-selectable="true"
              :edges-updatable="false"
              :fit-view-on-init="false"
              :default-viewport="{ x: 0, y: 0, zoom: 1 }"
              @init="handleFlowInit"
              @node-click="handleNodeClick"
            >
              <Background :gap="24" :size="1" pattern-color="color-mix(in srgb, currentColor 12%, transparent)" />
              <Controls position="bottom-left" :show-interactive="false">
                <ControlButton :title="t('config.organization.autoLayout')" @click="handleAutoLayout">
                  <RotateCcw class="h-3.5 w-3.5" />
                </ControlButton>
                <ControlButton
                  :title="isFlowFullscreen ? t('config.organization.exitFullscreen') : t('config.organization.openFullscreen')"
                  @click="isFlowFullscreen ? closeFlowFullscreen() : openFlowFullscreen()"
                >
                  <Minimize2 v-if="isFlowFullscreen" class="h-3.5 w-3.5" />
                  <Maximize2 v-else class="h-3.5 w-3.5" />
                </ControlButton>
              </Controls>

              <template #node-persona="nodeProps">
                <div
                  class="organization-tree-node flex flex-col items-center gap-1.5 transition"
                >
                  <div
                    class="flex h-14 w-14 items-center justify-center overflow-hidden rounded-full border-2 bg-base-200 text-base font-semibold text-base-content/70 transition"
                    :class="nodeProps.id === selectedPersonaId
                      ? 'border-primary'
                      : 'border-base-300 hover:border-base-content/25'"
                  >
                    <img
                      v-if="nodeProps.data.avatarUrl"
                      :src="nodeProps.data.avatarUrl"
                      :alt="nodeProps.data.name"
                      class="h-full w-full object-cover"
                    />
                    <span v-else>{{ nodeProps.data.avatarText }}</span>
                  </div>
                  <div
                    class="max-w-[110px] truncate text-center text-xs font-medium transition"
                    :class="nodeProps.id === selectedPersonaId ? 'text-primary' : 'text-base-content/80'"
                  >
                    {{ nodeProps.data.name }}
                  </div>
                </div>
              </template>
            </VueFlow>
          </div>
        </div>
      </Teleport>
    </div>
  </SettingsStickyLayout>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import dagre from "@dagrejs/dagre";
import { Background } from "@vue-flow/background";
import { ControlButton, Controls } from "@vue-flow/controls";
import {
  MarkerType,
  Position,
  VueFlow,
  type Edge as FlowEdge,
  type Node as FlowNode,
  type VueFlowStore,
} from "@vue-flow/core";
import "@vue-flow/core/dist/style.css";
import "@vue-flow/core/dist/theme-default.css";
import "@vue-flow/controls/dist/style.css";
import { Check, Maximize2, Minimize2, RotateCcw, Save } from "@lucide/vue";
import type { PersonaProfile } from "../../../../types/app";
import SettingsStickyLayout from "../../components/SettingsStickyLayout.vue";
import { useUnsavedChangesGuard } from "../../composables/use-unsaved-changes-guard";

const props = withDefaults(defineProps<{
  personas: PersonaProfile[];
  personaAvatarUrlMap?: Record<string, string>;
  saving?: boolean;
  saveRelations?: (updates: { agentId: string; childAgentIds: string[] }[]) => Promise<boolean>;
  setStatusAction?: (text: string) => void;
}>(), {
  personaAvatarUrlMap: () => ({}),
  saving: false,
  saveRelations: undefined,
  setStatusAction: undefined,
});

const { t } = useI18n();
const unsavedGuard = useUnsavedChangesGuard();

type PersonaRelationDraft = {
  id: string;
  childAgentIds: string[];
};
type PersonaFlowNodeData = {
  name: string;
  avatarUrl: string;
  avatarText: string;
};
type PersonaFlowNode = FlowNode<PersonaFlowNodeData>;
type PersonaFlowEdge = FlowEdge;

const NODE_WIDTH = 120;
const NODE_HEIGHT = 96;

const flowNodes = ref<PersonaFlowNode[]>([]);
const flowEdges = ref<PersonaFlowEdge[]>([]);
const flowInstance = ref<VueFlowStore | null>(null);
const isFlowFullscreen = ref(false);
const selectedPersonaId = ref("");
const saving = ref(false);

// ========== 组织成员与草稿 ==========

// 组织成员 = 可参与组织的人格；用户人格与系统人格不入树（与委托候选口径一致）。
const orgPersonas = computed(() =>
  (props.personas || []).filter((persona) => {
    const id = String(persona.id || "").trim();
    if (!id) return false;
    if (persona.isBuiltInUser) return false;
    if (id === "user-persona" || id === "system-persona") return false;
    return true;
  }),
);

const orgPersonaById = computed(() =>
  new Map(orgPersonas.value.map((persona) => [String(persona.id || "").trim(), persona] as const)),
);

function normalizeChildIds(value: unknown, selfId: string): string[] {
  if (!Array.isArray(value)) return [];
  const out: string[] = [];
  const seen = new Set<string>();
  const members = orgPersonaById.value;
  for (const item of value) {
    const id = String(item || "").trim();
    if (!id || id === selfId || seen.has(id) || !members.has(id)) continue;
    seen.add(id);
    out.push(id);
  }
  return out;
}

function cloneRelationDrafts(): PersonaRelationDraft[] {
  return orgPersonas.value.map((persona) => {
    const id = String(persona.id || "").trim();
    return { id, childAgentIds: normalizeChildIds(persona.childAgentIds, id) };
  });
}

function buildRelationSnapshot(drafts: PersonaRelationDraft[]): string {
  return JSON.stringify(
    drafts
      .map((draft) => ({ id: draft.id, childAgentIds: [...draft.childAgentIds].sort() }))
      .sort((left, right) => left.id.localeCompare(right.id, "zh-CN")),
  );
}

const relationDrafts = ref<PersonaRelationDraft[]>(cloneRelationDrafts());
const sourceRelationSnapshot = computed(() => buildRelationSnapshot(cloneRelationDrafts()));
const relationSnapshot = computed(() => buildRelationSnapshot(relationDrafts.value));
const relationDirty = computed(() => relationSnapshot.value !== sourceRelationSnapshot.value);

// ========== 选中节点与候选下级 ==========

const selectedPersona = computed(() =>
  orgPersonas.value.find((persona) => String(persona.id || "").trim() === selectedPersonaId.value) || null,
);

const selectedPersonaDisplayName = computed(() =>
  String(selectedPersona.value?.name || selectedPersonaId.value || "").trim(),
);

const selectedChildIds = computed(() =>
  relationDrafts.value.find((item) => item.id === selectedPersonaId.value)?.childAgentIds || [],
);

/** 某人格的全部祖先（沿 childAgentIds 反向找上级链），用于禁止把下级连成自己的祖先造成环。 */
function ancestorIdsOf(personaId: string): Set<string> {
  const ancestors = new Set<string>();
  const parentOf = new Map<string, string[]>();
  for (const draft of relationDrafts.value) {
    for (const childId of draft.childAgentIds) {
      const parents = parentOf.get(childId) || [];
      parents.push(draft.id);
      parentOf.set(childId, parents);
    }
  }
  const queue = [personaId];
  const visited = new Set<string>([personaId]);
  while (queue.length > 0) {
    const current = queue.shift()!;
    for (const parent of parentOf.get(current) || []) {
      if (visited.has(parent)) continue;
      visited.add(parent);
      ancestors.add(parent);
      queue.push(parent);
    }
  }
  return ancestors;
}

const selectedAncestorIdSet = computed(() => ancestorIdsOf(selectedPersonaId.value));

const candidatePersonas = computed(() => {
  const selectedId = selectedPersonaId.value;
  if (!selectedId) return [];
  const selectedIds = new Set(selectedChildIds.value);
  return orgPersonas.value
    .filter((persona) => {
      const id = String(persona.id || "").trim();
      if (!id || id === selectedId) return false;
      return !selectedAncestorIdSet.value.has(id);
    })
    .sort((left, right) => {
      const leftSelected = selectedIds.has(String(left.id || "").trim());
      const rightSelected = selectedIds.has(String(right.id || "").trim());
      if (leftSelected !== rightSelected) return leftSelected ? -1 : 1;
      return 0;
    });
});

const relationValidationMessage = computed(() => {
  // 校验：草稿沿 childAgentIds 是否成环（DFS 回边检测）
  const childMap = new Map(relationDrafts.value.map((d) => [d.id, d.childAgentIds]));
  const state = new Map<string, number>(); // 0=未访问 1=访问中 2=完成
  let cyclicId = "";
  function visit(id: string, stack: Set<string>): boolean {
    if (state.get(id) === 2) return false;
    if (stack.has(id)) {
      cyclicId = id;
      return true;
    }
    stack.add(id);
    for (const child of childMap.get(id) || []) {
      if (visit(child, stack)) return true;
    }
    stack.delete(id);
    state.set(id, 2);
    return false;
  }
  for (const draft of relationDrafts.value) {
    if (visit(draft.id, new Set())) {
      const name = String(orgPersonaById.value.get(cyclicId)?.name || cyclicId).trim();
      return t("config.organization.cycleError", { name });
    }
  }
  return "";
});

// ========== 头像 ==========

function avatarOf(id: string): string {
  return props.personaAvatarUrlMap[id] || "";
}

function initialOf(name: string): string {
  return String(name || "?").trim().slice(0, 1) || "?";
}

function avatarInfo(persona: PersonaProfile): { avatarUrl: string; avatarText: string } {
  const id = String(persona.id || "").trim();
  const name = String(persona.name || "").trim() || id;
  return {
    avatarUrl: avatarOf(id),
    avatarText: name.charAt(0) || id.charAt(0) || "人",
  };
}

// ========== 图构建 ==========

function childIdsOfDraft(personaId: string): string[] {
  return relationDrafts.value.find((item) => item.id === personaId)?.childAgentIds || [];
}

function buildPositions(): Map<string, { x: number; y: number }> {
  const graph = new dagre.graphlib.Graph();
  graph.setGraph({
    rankdir: "TB",
    align: "UL",
    nodesep: 72,
    ranksep: 120,
    edgesep: 20,
    marginx: 24,
    marginy: 24,
  });
  graph.setDefaultEdgeLabel(() => ({}));

  for (const persona of orgPersonas.value) {
    graph.setNode(String(persona.id || "").trim(), { width: NODE_WIDTH, height: NODE_HEIGHT });
  }
  for (const persona of orgPersonas.value) {
    const sourceId = String(persona.id || "").trim();
    for (const childId of childIdsOfDraft(sourceId)) {
      if (!graph.hasNode(childId)) continue;
      graph.setEdge(sourceId, childId);
    }
  }

  dagre.layout(graph);

  const positions = new Map<string, { x: number; y: number }>();
  for (const persona of orgPersonas.value) {
    const id = String(persona.id || "").trim();
    const node = graph.node(id);
    if (!node) continue;
    positions.set(id, {
      x: Math.round(Number(node.x || 0) - NODE_WIDTH / 2),
      y: Math.round(Number(node.y || 0) - NODE_HEIGHT / 2),
    });
  }
  return positions;
}

let lastFlowLayoutSignature = "";
function buildLayoutSignature(): string {
  return JSON.stringify(
    relationDrafts.value
      .map((d) => ({ id: d.id, childAgentIds: [...d.childAgentIds].sort() }))
      .sort((a, b) => a.id.localeCompare(b.id)),
  );
}

function buildNodes(): PersonaFlowNode[] {
  const positions = buildPositions();
  return orgPersonas.value.map((persona) => {
    const id = String(persona.id || "").trim();
    const name = String(persona.name || "").trim() || id;
    const avatar = avatarInfo(persona);
    return {
      id,
      type: "persona",
      position: positions.get(id) || { x: 0, y: 0 },
      draggable: true,
      sourcePosition: Position.Bottom,
      targetPosition: Position.Top,
      data: {
        name,
        avatarUrl: avatar.avatarUrl,
        avatarText: avatar.avatarText,
      },
    };
  });
}

function buildEdges(): PersonaFlowEdge[] {
  const edges: PersonaFlowEdge[] = [];
  const selected = selectedPersonaId.value;
  for (const persona of orgPersonas.value) {
    const sourceId = String(persona.id || "").trim();
    for (const childId of childIdsOfDraft(sourceId)) {
      const connected = !!selected && (sourceId === selected || childId === selected);
      edges.push({
        id: `${sourceId}-->${childId}`,
        source: sourceId,
        target: childId,
        type: "bezier",
        markerEnd: {
          type: MarkerType.ArrowClosed,
          color: connected ? "var(--color-primary)" : undefined,
        },
        class: connected ? "org-edge-active" : selected ? "org-edge-dim" : "",
        style: { strokeWidth: connected ? 2.2 : 1.4 },
        selectable: false,
      });
    }
  }
  return edges;
}

function syncFlowGraph(forceReset: boolean) {
  const nextSignature = buildLayoutSignature();
  const shouldRelayout = forceReset || nextSignature !== lastFlowLayoutSignature;
  const nextNodes = buildNodes();
  const existingPositions = new Map<string, { x: number; y: number }>();
  if (!shouldRelayout) {
    for (const node of flowNodes.value) existingPositions.set(node.id, node.position);
  }
  const syncedNodes: PersonaFlowNode[] = [];
  for (const node of nextNodes) {
    syncedNodes.push({ ...node, position: existingPositions.get(node.id) || node.position });
  }
  flowNodes.value = syncedNodes;
  flowEdges.value = buildEdges();
  lastFlowLayoutSignature = nextSignature;
}

// ========== 交互 ==========

function toggleChildPersona(childPersonaId: string) {
  const id = selectedPersonaId.value;
  if (!id) return;
  const draft = relationDrafts.value.find((item) => item.id === id);
  if (!draft) return;
  const childId = String(childPersonaId || "").trim();
  if (!childId || childId === id) return;
  const next = new Set(draft.childAgentIds);
  if (next.has(childId)) {
    next.delete(childId);
  } else if (candidatePersonas.value.some((persona) => String(persona.id || "").trim() === childId)) {
    next.add(childId);
  }
  draft.childAgentIds = normalizeChildIds(Array.from(next), id);
}

function switchSelectedPersona(nextId: string) {
  const trimmedId = String(nextId || "").trim();
  if (!trimmedId || trimmedId === selectedPersonaId.value) return;
  if (relationDirty.value) {
    const name = selectedPersonaDisplayName.value || t("config.organization.title");
    props.setStatusAction?.(t("config.organization.unsavedSwitchHint", { name }));
  }
  selectedPersonaId.value = trimmedId;
}

function handleNodeClick(payload: { node?: { id?: string } }) {
  const nextId = String(payload?.node?.id || "").trim();
  if (!nextId) return;
  switchSelectedPersona(nextId);
}

function restoreDraftsFromSaved() {
  relationDrafts.value = cloneRelationDrafts();
}

async function savePersonaRelations(): Promise<boolean> {
  if (!props.saveRelations || relationValidationMessage.value || !relationDirty.value) return false;
  saving.value = true;
  try {
    const updates = relationDrafts.value.map((draft) => ({
      agentId: draft.id,
      childAgentIds: [...draft.childAgentIds],
    }));
    const saved = await props.saveRelations(updates);
    if (!saved) return false;
    restoreDraftsFromSaved();
    return true;
  } finally {
    saving.value = false;
  }
}

// ========== 图生命周期 ==========

function handleFlowInit(instance: VueFlowStore) {
  flowInstance.value = instance;
  void fitFlowIntoView(0.08);
}

async function fitFlowIntoView(padding: number) {
  await nextTick();
  await flowInstance.value?.viewportHelper.fitView({ padding, maxZoom: 1 });
}

async function handleAutoLayout() {
  syncFlowGraph(true);
  await fitFlowIntoView(0.16);
  props.setStatusAction?.(t("config.organization.autoLayoutDone"));
}

async function openFlowFullscreen() {
  if (isFlowFullscreen.value) return;
  isFlowFullscreen.value = true;
  await fitFlowIntoView(0.08);
}

async function closeFlowFullscreen() {
  if (!isFlowFullscreen.value) return;
  isFlowFullscreen.value = false;
  await fitFlowIntoView(0.08);
}

function handleWindowKeydown(event: KeyboardEvent) {
  if (event.key !== "Escape" || !isFlowFullscreen.value) return;
  event.preventDefault();
  void closeFlowFullscreen();
}

// ========== watchers ==========

watch(
  () => orgPersonas.value.map((item) => String(item.id || "").trim()).join("|"),
  () => {
    if (!orgPersonas.value.some((item) => String(item.id || "").trim() === selectedPersonaId.value)) {
      selectedPersonaId.value = String(orgPersonas.value[0]?.id || "").trim();
    }
    syncFlowGraph(false);
  },
  { immediate: true },
);

watch(
  () => sourceRelationSnapshot.value,
  () => {
    if (relationDirty.value) return;
    restoreDraftsFromSaved();
  },
);

watch(
  () => relationSnapshot.value,
  () => {
    syncFlowGraph(false);
  },
);

// 选中节点变化时重建边：相连边高亮、其余淡化
watch(selectedPersonaId, () => {
  flowEdges.value = buildEdges();
});

onMounted(() => {
  window.addEventListener("keydown", handleWindowKeydown);
  const unregister = unsavedGuard.registerDirtyChecker("organization-relations", {
    // 高于全局 config checker：「保存并离开」先落在组织关系草稿上。
    priority: 10,
    isDirty: () => relationDirty.value,
    title: t("config.unsavedConfirm.title"),
    message: t("config.unsavedConfirm.message"),
    onDiscard: () => {
      restoreDraftsFromSaved();
    },
    onSave: async () => {
      return await savePersonaRelations();
    },
  });
  onUnmounted(unregister);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleWindowKeydown);
});
</script>

<style scoped>
.organization-tree-flow {
  background: transparent;
}

.organization-tree-node {
  width: 120px;
}

/* 选中节点时：相连边高亮、其余边淡化，避免多父连线汇成一团看不清 */
.organization-tree-flow :deep(.org-edge-active path) {
  stroke: var(--color-primary) !important;
}

.organization-tree-flow :deep(.org-edge-dim) {
  opacity: 0.25;
}

.organization-tree-flow :deep(.vue-flow__edge) {
  transition: opacity 0.15s ease;
}

/* Controls 按钮默认是白底深色图标，深色主题下看不见；改成跟随 DaisyUI 主题色。
   lucide 是描边图标，只设 stroke/color，不能设 fill（否则被填成实心圆点）。 */
.organization-tree-flow :deep(.vue-flow__controls-button) {
  background: var(--color-base-200);
  color: var(--color-base-content);
  border-color: var(--color-base-300);
}

.organization-tree-flow :deep(.vue-flow__controls-button:hover) {
  background: var(--color-base-300);
}

.organization-tree-flow :deep(.vue-flow__controls) {
  box-shadow: none;
  border: 1px solid color-mix(in srgb, currentColor 12%, transparent);
  border-radius: 1rem;
  overflow: hidden;
}
</style>
