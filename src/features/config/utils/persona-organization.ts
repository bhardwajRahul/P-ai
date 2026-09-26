import type { PersonaProfile } from "../../../types/app";

type RelationSource = { id: string; childAgentIds: string[] };

/**
 * 计算某人格的全部祖先（沿 childAgentIds 反向找上级链，BFS 到顶）。
 * 用于防环：把祖先设为自己的下级会造成 A→B→A 回环，因此候选下级列表必须排除祖先。
 * 人格是多父图（可有多个上级），所以这里收集整棵祖先链而非只找直接上级。
 *
 * @param personaId 目标人格 id
 * @param source 关系来源：默认传 PersonaProfile[]（按已保存数据算）；
 *   也可传形如 { id, childAgentIds }[] 的草稿态关系（组织页未保存编辑时用）。
 */
export function personaAncestorIds(
  personaId: string,
  source: PersonaProfile[] | RelationSource[],
): Set<string> {
  const target = String(personaId || "").trim();
  const ancestors = new Set<string>();
  if (!target) return ancestors;

  // 反向映射：childId → 所有上级 id
  const parentOf = new Map<string, string[]>();
  for (const item of source) {
    const parentId = String((item as RelationSource).id || "").trim();
    if (!parentId) continue;
    for (const raw of (item as RelationSource).childAgentIds || []) {
      const childId = String(raw || "").trim();
      if (!childId) continue;
      const parents = parentOf.get(childId) || [];
      parents.push(parentId);
      parentOf.set(childId, parents);
    }
  }

  const queue = [target];
  const visited = new Set<string>([target]);
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
