// Axolotl Viewer — 前端逻辑（只读图库浏览）
// 数据流：open_graph 一次性返回全量快照 → 前端内存持有 → 渲染聚焦子图

"use strict";

const EXPAND_LIMIT = 400; // 单次渲染子图节点上限
const invoke = window.__TAURI__.core.invoke;
const dialog = window.__TAURI__.dialog;

// ── 状态 ──
let graph = null; // { vertices: Map<id, Node>, edges: Map<"f->t", Edge>, bySid: Map<sid, id> }
let cy = null;
let selectedId = null;

const DOMAIN_COLORS = {
  knowledge: "#2563eb",
  emotion: "#dc2626",
  task: "#059669",
  mixed: "#7c3aed",
  default: "#64748b",
};
const KIND_COLORS = {
  has_member: "#94a3b8",
  feedback: "#ef4444",
  default: "#cbd5e1",
};

const $ = (sel) => document.querySelector(sel);

// ── 打开文件 ──
$("#btn-open").addEventListener("click", async () => {
  try {
    const path = await dialog.open({
      title: "打开图库文件 (.axeb)",
      filters: [{ name: "Axolotl 图库", extensions: ["axeb"] }],
      multiple: false,
    });
    if (!path) return;
    await loadGraph(path);
  } catch (e) {
    setStatus("打开失败: " + e, true);
  }
});

// 拖拽 .axeb 到窗口（Tauri 2 官方 drag-drop 事件；窗口默认拦截 DOM drop）
window.__TAURI__.event.listen("tauri://drag-drop", (e) => {
  const p = e.payload?.paths?.[0];
  if (p) loadGraph(p);
});

async function loadGraph(path) {
  setStatus("正在加载 " + path.split("/").pop() + " …");
  try {
    const snap = await invoke("open_graph", { path });
    graph = {
      vertices: new Map(),
      edges: new Map(),
      bySid: new Map(),
    };
    for (const v of snap.vertices) {
      graph.vertices.set(v.id, v);
      graph.bySid.set(v.sid, v.id);
    }
    for (const e of snap.edges) {
      graph.edges.set(`${e.from}->${e.to}`, e);
    }
    $("#file-info").textContent = snap.path;
    $("#stats").textContent = `● ${snap.vertex_count} 节点 / ${snap.edge_count} 边`;
    buildFilters();
    renderInitial();
    setStatus(`已加载 ${snap.path.split("/").pop()}（${snap.vertex_count} 节点）`);
  } catch (e) {
    setStatus("加载失败: " + e, true);
  }
}

// ── 过滤器 ──
function buildFilters() {
  const domains = new Set();
  const types = new Set();
  for (const v of graph.vertices.values()) {
    domains.add(v.props.domain ?? "knowledge");
    types.add(v.props.type ?? "concept");
  }
  const fill = (sel, set) => {
    const el = $(sel);
    const cur = el.value;
    el.innerHTML = `<option value="">全部${sel.includes("domain") ? " domain" : "类型"}</option>`;
    for (const d of [...set].sort()) {
      const o = document.createElement("option");
      o.value = d;
      o.textContent = d;
      el.appendChild(o);
    }
    el.value = cur;
  };
  fill("#filter-domain", domains);
  fill("#filter-type", types);
}

// ── 图渲染（cytoscape）──
const CYTOSCAPE_STYLE = [
  {
    selector: "node",
    style: {
      width: "mapData(degree, 0, 12, 14, 46)",
      height: "mapData(degree, 0, 12, 14, 46)",
      "background-color": (el) =>
        DOMAIN_COLORS[el.data("domain")] ?? DOMAIN_COLORS.default,
      label: (el) => {
        const l = el.data("label") || "";
        return l.length > 18 ? l.slice(0, 17) + "…" : l;
      },
      "font-size": 11,
      color: "#334155",
      "text-valign": "bottom",
      "text-margin-y": 4,
      "border-width": 1.5,
      "border-color": "#ffffff",
    },
  },
  {
    selector: "node.selected",
    style: {
      "border-width": 3,
      "border-color": "#f59e0b",
      "z-index": 100,
    },
  },
  {
    selector: "node.neighbor",
    style: { opacity: 1, "z-index": 90 },
  },
  {
    selector: "node.faded",
    style: { opacity: 0.18 },
  },
  {
    selector: "edge",
    style: {
      width: 1.6,
      "line-color": (el) => KIND_COLORS[el.data("kind")] ?? KIND_COLORS.default,
      "curve-style": "bezier",
      "target-arrow-shape": "triangle",
      "target-arrow-color": (el) => KIND_COLORS[el.data("kind")] ?? KIND_COLORS.default,
      "arrow-scale": 0.7,
      "z-index": 1,
    },
  },
  {
    selector: "edge.faded",
    style: { opacity: 0.08 },
  },
  {
    selector: "edge.neighbor",
    style: { opacity: 0.9, width: 2.2, "z-index": 50 },
  },
];

function initCy() {
  cy = cytoscape({
    container: $("#cy"),
    style: CYTOSCAPE_STYLE,
    layout: { name: "cose", animate: false, nodeRepulsion: 4000, idealEdgeLength: 90 },
    minZoom: 0.05,
    maxZoom: 4,
    wheelSensitivity: 0.25,
  });

  cy.on("tap", "node", (evt) => selectNode(evt.target.id()));
  cy.on("dbltap", "node", (evt) => expand(evt.target.id()));
  cy.on("tap", (evt) => {
    if (evt.target === cy) clearSelection();
  });
}

function renderInitial() {
  if (!cy) initCy();
  cy.elements().remove();
  selectedId = null;
  $("#detail-body").hidden = true;
  $("#detail-empty").hidden = false;

  // 起始点：lobster_root；没有则取入度最高的节点
  let startId = graph.bySid.get("lobster_root");
  if (startId == null) {
    startId = highestDegreeVertex();
  }
  if (startId == null) {
    $("#node-list").innerHTML = `<div class="empty">图库为空</div>`;
    setStatus("图库为空");
    return;
  }
  addSubgraph(startId, 2);
  cy.fit(undefined, 50);
  renderList();
}

function highestDegreeVertex() {
  const deg = new Map();
  for (const e of graph.edges.values()) {
    deg.set(e.from, (deg.get(e.from) ?? 0) + 1);
  }
  let best = null;
  for (const [id, v] of graph.vertices) {
    const d = deg.get(id) ?? 0;
    if (v.sid !== "lobster_root" && (best === null || d > best[1])) best = [id, d];
  }
  return best ? best[0] : graph.vertices.keys().next().value;
}

// 从 centerId 出发展开 depth 跳子图（已显示节点跳过）
function addSubgraph(centerId, depth) {
  if (!graph.vertices.has(centerId)) return;
  const inCy = new Set(cy.nodes().map((n) => n.id()));
  const queue = [[centerId, 0]];
  const seen = new Set([String(centerId)]);
  const nodeIds = [];
  const edgeKeys = [];
  while (queue.length) {
    const [nid, d] = queue.shift();
    if (d > depth) continue;
    nodeIds.push(String(nid));
    for (const e of outgoingOf(nid)) {
      edgeKeys.push(`${e.from}->${e.to}`);
      if (d < depth && !seen.has(String(e.to))) {
        seen.add(String(e.to));
        queue.push([e.to, d + 1]);
      }
    }
  }
  const freshNodes = nodeIds.filter((id) => !inCy.has(id));
  if (freshNodes.length > EXPAND_LIMIT) {
    setStatus(`子图将新增 ${freshNodes.length} 个节点（超过上限 ${EXPAND_LIMIT}），已停止展开`, true);
    return;
  }
  const els = [];
  for (const nid of nodeIds) {
    if (inCy.has(nid)) continue;
    const v = graph.vertices.get(Number(nid));
    els.push({
      data: {
        id: nid,
        label: v.label,
        sid: v.sid,
        domain: v.props.domain ?? "knowledge",
        type: v.props.type ?? "",
        degree: 0,
      },
    });
  }
  for (const k of edgeKeys) {
    if (cy.getElementById(k).length) continue;
    const e = graph.edges.get(k);
    if (!e) continue;
    els.push({
      data: {
        id: k,
        source: String(e.from),
        target: String(e.to),
        kind: e.kind,
        weight: e.weight,
      },
    });
  }
  if (els.length) {
    cy.add(els);
    recomputeDegrees();
    // 增量布局：让新节点落入位置
    cy.layout({ name: "cose", animate: true, animationDuration: 350, nodeRepulsion: 4000, idealEdgeLength: 90 }).run();
  }
}

function outgoingOf(nid) {
  const out = [];
  for (const e of graph.edges.values()) if (e.from === nid) out.push(e);
  return out;
}

function recomputeDegrees() {
  const deg = new Map();
  for (const e of graph.edges.values()) {
    deg.set(e.from, (deg.get(e.from) ?? 0) + 1);
    deg.set(e.to, (deg.get(e.to) ?? 0) + 1);
  }
  cy.nodes().forEach((n) => n.data("degree", deg.get(Number(n.id())) ?? 0));
}

function expand(nid) {
  if (!graph.vertices.has(Number(nid))) return;
  addSubgraph(Number(nid), 1);
}

// ── 选中 / 详情 ──
function selectNode(nid) {
  selectedId = nid;
  const node = cy.getElementById(nid);
  cy.elements().removeClass("selected neighbor faded");
  const nbrs = node.neighborhood();
  node.addClass("selected");
  nbrs.addClass("neighbor");
  cy.elements().not(node.union(nbrs)).addClass("faded");
  renderDetail(nid);
  updateListActive(nid);
}

function clearSelection() {
  selectedId = null;
  cy.elements().removeClass("selected neighbor faded");
  $("#detail-body").hidden = true;
  $("#detail-empty").hidden = false;
}

function renderDetail(nid) {
  const v = graph.vertices.get(Number(nid));
  if (!v) return;
  $("#detail-empty").hidden = true;
  $("#detail-body").hidden = false;
  $("#detail-title").textContent = v.label;
  $("#detail-sub").textContent = `#${v.sid} · id=${nid}`;

  // 属性表
  const tb = $("#detail-props");
  tb.innerHTML = "";
  for (const [k, val] of Object.entries(v.props)) {
    const tr = document.createElement("tr");
    const tdK = document.createElement("td");
    tdK.textContent = k;
    const tdV = document.createElement("td");
    let text = typeof val === "string" ? val : JSON.stringify(val);
    if (k === "content" && text.length > 400) {
      const short = text.slice(0, 400);
      const details = document.createElement("details");
      const summary = document.createElement("summary");
      summary.textContent = short + "…";
      const pre = document.createElement("div");
      pre.textContent = text.slice(400);
      pre.style.cssText = "white-space:pre-wrap;margin-top:6px;color:#475569";
      details.appendChild(summary);
      details.appendChild(pre);
      tdV.appendChild(details);
    } else {
      tdV.textContent = text.length > 600 ? text.slice(0, 600) + "…" : text;
    }
    tr.appendChild(tdK);
    tr.appendChild(tdV);
    tb.appendChild(tr);
  }

  // 出/入邻居
  const outs = [];
  const ins = [];
  for (const e of graph.edges.values()) {
    if (e.from === v.id) outs.push(e);
    if (e.to === v.id) ins.push(e);
  }
  $("#detail-out-count").textContent = `(${outs.length})`;
  $("#detail-in-count").textContent = `(${ins.length})`;
  fillNeighbors($("#detail-out"), outs, true);
  fillNeighbors($("#detail-in"), ins, false);
}

function fillNeighbors(ul, edges, isOut) {
  ul.innerHTML = "";
  edges.sort((a, b) => a.kind.localeCompare(b.kind));
  for (const e of edges) {
    const target = isOut ? e.to : e.from;
    const v = graph.vertices.get(target);
    if (!v) continue;
    const li = document.createElement("li");
    const dot = document.createElement("span");
    dot.className = "dot";
    dot.style.background = DOMAIN_COLORS[v.props.domain ?? "default"] ?? DOMAIN_COLORS.default;
    const txt = document.createElement("span");
    txt.textContent = v.label;
    txt.style.cssText = "overflow:hidden;text-overflow:ellipsis";
    const tag = document.createElement("span");
    tag.className = "kind-tag";
    tag.textContent = e.kind || "(无kind)";
    li.appendChild(dot);
    li.appendChild(txt);
    li.appendChild(tag);
    li.title = v.sid;
    li.addEventListener("click", () => {
      // 若不在图上，先展开该节点
      if (!cy.getElementById(String(target)).length) addSubgraph(target, 1);
      cy.getElementById(String(target)).select();
      selectNode(String(target));
      cy.center(cy.getElementById(String(target)));
    });
    ul.appendChild(li);
  }
}

// ── 节点列表（左栏）──
function renderList() {
  const q = ($("#search").value || "").trim().toLowerCase();
  const fd = $("#filter-domain").value;
  const ft = $("#filter-type").value;
  const ul = $("#node-list");
  ul.innerHTML = "";

  let items = [...graph.vertices.values()];
  if (fd) items = items.filter((v) => (v.props.domain ?? "knowledge") === fd);
  if (ft) items = items.filter((v) => (v.props.type ?? "concept") === ft);
  if (q) items = items.filter((v) => v.label.toLowerCase().includes(q) || v.sid.toLowerCase().includes(q));
  items.sort((a, b) => a.sid.localeCompare(b.sid));
  if (items.length > 500) items = items.slice(0, 500);

  for (const v of items) {
    const div = document.createElement("div");
    div.className = "node-item";
    div.dataset.id = String(v.id);
    const dot = document.createElement("span");
    dot.className = "dot";
    dot.style.background = DOMAIN_COLORS[v.props.domain ?? "default"] ?? DOMAIN_COLORS.default;
    const label = document.createElement("span");
    label.textContent = v.label;
    const nid = document.createElement("span");
    nid.className = "nid";
    nid.textContent = v.sid;
    div.appendChild(dot);
    div.appendChild(label);
    div.appendChild(nid);
    div.addEventListener("click", () => {
      if (!cy.getElementById(String(v.id)).length) addSubgraph(v.id, 1);
      cy.getElementById(String(v.id)).select();
      selectNode(String(v.id));
      cy.center(cy.getElementById(String(v.id)));
    });
    ul.appendChild(div);
  }
  if (!items.length) ul.innerHTML = `<div class="empty">无匹配节点</div>`;
}

function updateListActive(nid) {
  document.querySelectorAll(".node-item").forEach((el) => {
    el.classList.toggle("active", el.dataset.id === String(nid));
  });
}

["search", "filter-domain", "filter-type"].forEach((id) =>
  $("#" + id).addEventListener("input", renderList)
);

// ── 状态栏 ──
function setStatus(msg, isError = false) {
  const el = $("#status");
  el.textContent = msg;
  el.style.color = isError ? "#dc2626" : "";
}

// 初始就绪
setStatus("点击「打开图库…」或拖拽 .axeb 文件到窗口");
