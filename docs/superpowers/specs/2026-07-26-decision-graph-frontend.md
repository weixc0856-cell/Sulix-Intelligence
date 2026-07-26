# Decision Graph Frontend — Design Spec v1.1

> 基于后端 `GET /api/projections/decision-graph` 端点的前端可视化。
> 以 Decision 为中心，定位为 **Cognitive State Explorer（认知状态探索器）**，而非单纯的关系图。

**Goal:** 在 `/intelligence/graph` 页面中，用 Cytoscape.js 渲染决策关系图，支持渐进展开、证据预览、节点详情面板。

**Architecture:** Astro SSR → JSON script injection → Cytoscape.js (npm) → Graph State Store → 渐进展开 API。

**Tech Stack:** Astro 5 + Tailwind + Cytoscape.js (npm) + cytoscape-dagre + cytoscape-cose

---

## Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `src/pages/intelligence/graph.astro` | Create | 主页面（Intelligence 下，非 Decisions） |
| `src/features/decision-graph/types.ts` | Create | Graph 类型定义 |
| `src/features/decision-graph/graph-state.ts` | Create | 图状态管理 |
| `src/features/decision-graph/graph-layout.ts` | Create | Cytoscape 布局配置 |
| `src/features/decision-graph/graph-style.ts` | Create | 节点/边样式 |
| `src/features/decision-graph/graph-api.ts` | Create | 后端 API 调用 |
| `src/features/decision-graph/graph.ts` | Create | Cytoscape 初始化 + 交互 |
| `src/lib/navigation.ts` | Modify | 添加 Intelligence → Graph 导航 |

---

## 1. Types (`src/features/decision-graph/types.ts`)

```typescript
export interface GraphNode {
  id: string;
  artifact_id: string;
  node_type: 'observation' | 'signal' | 'thesis' | 'decision' | 'outcome' | 'reflection' | 'memory';
  title: string;
  summary?: string;
  created_at: number;
  metrics: {
    confidence?: number;
    impact?: number;
    evidence_count?: number;
    uncertainty?: number;
  };
  state: {
    status: string;
    lifecycle: 'active' | 'resolved' | 'archived';
  };
}

export interface GraphEdge {
  id: string;
  source: string;
  target: string;
  relation: 'supports' | 'contradicts' | 'caused' | 'validated' | 'invalidated' | 'learned';
  strength: number;
  metadata: {
    evidence_count?: number;
  };
}

export interface GraphResponse {
  projection: string;
  root: { id: string; node_type: string };
  nodes: GraphNode[];
  edges: GraphEdge[];
  generated_at: number;
}
```

---

## 2. Graph State (`src/features/decision-graph/graph-state.ts`)

```typescript
export interface GraphState {
  selectedNodeId: string | null;
  expandedNodes: Set<string>;
  layout: 'flow' | 'map' | 'timeline';
  filters: {
    types: string[];
  };
}
```

---

## 3. Data Injection

不使用 `data-*` 属性注入大 JSON。

**`graph.astro`**:

```astro
<script type="application/json" id="decision-graph-data">{JSON.stringify(graphData)}</script>
```

**Client JS**:

```typescript
const data: GraphResponse = JSON.parse(
  document.getElementById('decision-graph-data')!.textContent!
);
```

---

## 4. Cytoscape (npm, 非 CDN)

`package.json`:
```json
{
  "dependencies": {
    "cytoscape": "^3.30.4",
    "cytoscape-dagre": "^2.5.0"
  }
}
```

**`src/features/decision-graph/graph-layout.ts`**:

```typescript
import cytoscape from 'cytoscape';
import dagre from 'cytoscape-dagre';

cytoscape.use(dagre);

export const LAYOUTS: Record<string, cytoscape.LayoutOptions> = {
  flow: { name: 'dagre', rankDir: 'TB', spacingFactor: 1.5 },
  map: { name: 'cose', animate: true },
  timeline: { name: 'preset', positions: (node: any) => ({
    x: parseInt(node.data('id').split('-')[1]) * 200,
    y: { observation: 0, signal: 100, thesis: 200, decision: 300, outcome: 400, reflection: 500, memory: 600 }[node.data('type')] || 300,
  })},
};
```

---

## 5. Security (XSS 防护)

所有用户/LLM 生成内容使用 `textContent` 而非 `innerHTML`：

```typescript
function escapeHtml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');
}

// 构建详情面板
const titleEl = document.createElement('h2');
titleEl.textContent = title;  // 安全

const summaryEl = document.createElement('p');
summaryEl.textContent = summary || '';  // 安全
```

---

## 6. Progressive Expansion

新增后端 API:
```
GET /api/projections/decision-graph/:id/neighbors
```

返回以该节点为中心的子图。

前端点击展开：
```typescript
cy.on('dblclick', 'node', async (evt) => {
  const nodeId = evt.target.id();
  const neighbors = await fetchNeighbors(nodeId);
  cy.add([...neighbors.nodes, ...neighbors.edges]);
  cy.layout({ name: 'dagre', fit: true }).run();
});
```

---

## 7. Navigation

`src/lib/navigation.ts`:

Intelligence 分组下增加：
```typescript
{ label: 'Decision Graph', href: '/intelligence/graph', icon: 'account_tree' },
```

路由在 `/intelligence/graph`（非 `/intelligence/decisions/graph`）。

---

## 8. Layout

```
────────────────────────────────────
 Intelligence / Decision Graph
────────────────────────────────────

[Layout: Flow ▼]  [{graphData.nodes.length} nodes · {graphData.edges.length} edges]

       ┌─────────────────────────────────────┐
       │                                     │
       │           Cytoscape Canvas          │
       │                                     │
       │   Signal ──► Decision ──► Outcome   │
       │                                     │
       └──────────────┬──────────────────────┘
                      │ click node
                      ▼
       ┌─────────────────────────────────────┐
       │          Detail Panel               │
       │                                     │
       │  DEC-001                            │
       │  AI Agent Market Entry              │
       │                                     │
       │  Confidence  ████████░░  82%        │
       │                                     │
       │  Why?  3 supporting signals         │
       │                                     │
       │  Evidence:                          │
       │  • OpenAI agent release             │
       │  • Enterprise adoption trend        │
       │                                     │
       │  Outcome: Pending                   │
       │  Learning: Awaiting validation      │
       └─────────────────────────────────────┘
```

---

## 9. File Structure

```
src/
├── features/
│   └── decision-graph/
│       ├── types.ts
│       ├── graph-state.ts
│       ├── graph-layout.ts
│       ├── graph-style.ts
│       ├── graph-api.ts
│       └── graph.ts
├── pages/
│   └── intelligence/
│       └── graph.astro         ← 新页面
└── lib/
    └── navigation.ts           ← 修改
```

---

## 10. Non-Goals (v1.1)

| 功能 | 状态 | 备注 |
|------|------|------|
| ✅ 可视化渲染 | v1.1 必做 | Cytoscape.js |
| ✅ 节点点击展开详情 | v1.1 必做 | 安全 DOM API |
| ✅ 渐进展开邻居 | v1.1 必做 | 后端 `/neighbors` 端点 |
| ✅ 证据预览 | v1.1 必做 | Detail panel 展示 |
| ✅ 布局切换 | v1.1 必做 | flow / map / timeline |
| ❌ 搜索/筛选 | 延期 | v1.2 |
| ❌ 实时更新 | 延期 | v2.0 |
| ❌ 节点编辑 | 延期 | v2.0 |

---

## Verification

```bash
# 后端
curl http://127.0.0.1:8787/api/projections/decision-graph?limit=5

# 前端
cd D:\Project\intel-web
npm install cytoscape cytoscape-dagre
npm run dev
# 访问 http://localhost:4321/intelligence/graph
```
