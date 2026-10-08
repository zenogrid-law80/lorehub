/* Shared graph presentation state, independent of CI definition revisions. */
class CiLayoutSession {
  constructor(scope, send, changed = () => {}) {
    this.scope = scope; this.send = send; this.changed = changed;
    this.positions = {}; this.pending = {}; this.ready = false;
    this.loading = false; this.saving = false; this.resetting = false; this.retryReset = false; this.error = ""; this.timer = null;
  }
  async load() {
    if (this.loading) return;
    this.loading = true; this.ready = false; this.error = ""; this.changed();
    try { this.positions = (await this.send("GET", this.scope)).positions; this.ready = true; }
    catch (error) { this.error = error.message; }
    finally { this.loading = false; this.changed(); }
  }
  move(key, point) {
    if (!this.ready || this.loading || this.resetting) return;
    this.retryReset = false;
    this.positions = { ...this.positions, [key]: point };
    this.pending = { ...this.pending, [key]: point }; this.error = "";
    clearTimeout(this.timer); this.timer = setTimeout(() => void this.flush(), 250);
    this.changed();
  }
  async flush() {
    clearTimeout(this.timer);
    if (this.saving || !Object.keys(this.pending).length) return;
    const patch = this.pending; this.pending = {}; this.saving = true; this.error = ""; this.changed();
    try {
      const saved = await this.send("POST", { ...this.scope, positions: patch });
      this.positions = { ...saved.positions, ...this.pending };
    } catch (error) {
      this.pending = { ...patch, ...this.pending }; this.error = error.message;
    } finally {
      this.saving = false; this.changed();
      if (!this.error && Object.keys(this.pending).length) void this.flush();
    }
  }
  async reset() {
    if (!this.ready || this.loading || this.saving || Object.keys(this.pending).length) return false;
    this.saving = true; this.resetting = true; this.retryReset = false; this.error = ""; this.changed();
    try { this.positions = (await this.send("POST", { ...this.scope, reset: true })).positions; return true; }
    catch (error) { this.error = error.message; this.retryReset = true; return false; }
    finally { this.saving = false; this.resetting = false; this.changed(); }
  }
}

const ciLayout = { session: null, sessions: new Map(), key: null, nodes: [], generation: 0, drag: null, suppressClick: false };
for (const [key, ko, zh] of [
  ["Reset layout", "자동 배치로 초기화", "重置布局"], ["Retry layout", "배치 저장 다시 시도", "重试布局"],
  ["Loading layout…", "배치 불러오는 중…", "正在加载布局…"],
  ["Saving layout…", "배치 저장 중…", "正在保存布局…"],
  ["Layout saved to database", "배치가 DB에 저장됨", "布局已保存到数据库"],
  ["Automatic layout", "자동 배치", "自动布局"],
  ["Layout unavailable", "배치 불러오기 실패", "无法加载布局"],
  ["Layout not saved", "배치 저장 실패", "布局未保存"],
  ["Drag nodes to move; Alt + arrow keys also work. Layout does not change execution order.", "노드를 드래그하거나 Alt + 방향키로 이동하세요. 배치는 실행 순서에 영향을 주지 않습니다.", "拖动节点或按 Alt + 方向键移动。布局不改变执行顺序。"],
]) { I18N.en[key] = key; I18N.ko[key] = ko; I18N["zh-CN"][key] = zh; }

function ciLayoutScope() {
  const selected = selectedCiPipeline(state.repositoryConfigEditing ? state.repositoryConfigDraft : state.repositoryConfigModel);
  return { branch: elements["repository-config-branch"].value,
    graph: JSON.stringify(ciVisual.scope === "overview" ? ["overview"] : ["detail", selected?.legacy ? null : selected?.pipeline.name]) };
}

function renderCiLayoutStatus() {
  const session = ciLayout.session, status = ciElement("ci-layout-status");
  if (!status) return;
  status.textContent = !session || session.loading ? t("Loading layout…")
    : session.error ? `${t(session.ready ? "Layout not saved" : "Layout unavailable")}: ${session.error}`
      : session.saving || Object.keys(session.pending).length ? t("Saving layout…")
        : t(Object.keys(session.positions).length ? "Layout saved to database" : "Automatic layout");
  ciElement("ci-layout-retry").hidden = !session?.error;
  ciElement("ci-layout-retry").disabled = Boolean(session?.loading || session?.saving);
  ciElement("ci-layout-reset").disabled = !session?.ready || session.saving || !ciLayout.nodes.length || Object.keys(session.pending).length > 0 || state.repositoryConfigSaving;
}

function prepareCiLayout() {
  const graph = elements["repository-config-stage-graph"], generation = ++ciLayout.generation;
  ciLayout.drag = null; ciLayout.nodes = [];
  graph.classList.remove("ci-free-layout"); graph.style.width = ""; graph.style.height = "";
  const scope = ciLayoutScope(), name = state.repositoryConfigName;
  const key = JSON.stringify([state.user?.id, name, scope.branch, scope.graph]);
  if (ciLayout.key !== key) {
    // An old branch's pending request retains its original URL and scope.
    void ciLayout.session?.flush(); ciLayout.key = key;
    const url = `/api/v1/repositories/${encodeURIComponent(name)}/ci-config/layout`;
    const session = ciLayout.sessions.get(key) ?? new CiLayoutSession(scope, (method, body) => method === "GET"
      ? api(`${url}?${new URLSearchParams(body)}`)
      : api(url, { method, headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() }, body: JSON.stringify(body) }),
    () => { if (ciLayout.session === session) { renderCiLayoutStatus(); if (!ciLayout.drag) paintCiLayout(); } });
    ciLayout.session = session; ciLayout.sessions.set(key, session);
    if (!session.saving && !Object.keys(session.pending).length) void session.load();
    session.request = state.repositoryConfigRequest;
  } else if (ciLayout.session.request !== state.repositoryConfigRequest) {
    ciLayout.session.request = state.repositoryConfigRequest;
    if (!ciLayout.session.saving && !Object.keys(ciLayout.session.pending).length) void ciLayout.session.load();
  }
  renderCiLayoutStatus();
  window.requestAnimationFrame(() => {
    if (generation !== ciLayout.generation || !graph.isConnected || state.repositoryConfigMode !== "visual") return;
    const model = state.repositoryConfigEditing ? state.repositoryConfigDraft : state.repositoryConfigModel;
    const entries = ciPipelineEntries(model), selected = selectedCiPipeline(model);
    const nodes = [];
    const add = (node, handle, key, parent, minY = 0) => {
      const rect = node.getBoundingClientRect(), origin = parent.getBoundingClientRect();
      const position = { x: (rect.left - origin.left) / ciEditor.zoom - parent.clientLeft,
        y: (rect.top - origin.top) / ciEditor.zoom - parent.clientTop };
      nodes.push({ node, handle, key: JSON.stringify(key), parent, minY, original: position, position, width: node.offsetWidth });
    };
    if (ciVisual.scope === "overview") {
      for (const card of graph.querySelectorAll(".ci-pipeline-card")) {
        const entry = entries[Number(card.dataset.pipelineIndex)];
        add(card, card, ["pipeline", entry.legacy ? null : entry.pipeline.name], graph);
      }
    } else if (selected) {
      for (const column of graph.querySelectorAll(".config-stage-column")) {
        const header = column.querySelector(".config-stage-header"), stage = selected.pipeline.stages[Number(header.dataset.stageIndex)];
        add(column, header, ["stage", stage], graph);
        for (const job of column.querySelectorAll(".config-job-card")) {
          add(job, job, ["job", stage, job.dataset.jobName], column, header.offsetTop + header.offsetHeight + 12);
        }
      }
    }
    ciLayout.nodes = nodes;
    for (const item of nodes) {
      item.node.style.position = "absolute"; item.node.style.width = `${item.width}px`;
      item.handle.classList.add("ci-layout-handle"); item.handle.dataset.layoutKey = item.key;
    }
    graph.classList.add("ci-free-layout"); paintCiLayout(); renderCiLayoutStatus();
  });
}

function clampCiNode(point, minY = 0) {
  return { x: Math.round(Math.max(0, Math.min(50000, point.x))), y: Math.round(Math.max(minY, Math.min(50000, point.y))) };
}

function paintCiLayout() {
  if (!ciLayout.nodes.length) return;
  for (const item of ciLayout.nodes) {
    const point = ciLayout.drag?.item === item ? item.position : ciLayout.session?.positions[item.key] ?? item.original;
    item.position = clampCiNode(point, item.minY);
    item.node.style.left = `${item.position.x}px`; item.node.style.top = `${item.position.y}px`;
  }
  const graph = elements["repository-config-stage-graph"];
  for (const column of graph.querySelectorAll(".config-stage-column")) {
    const children = ciLayout.nodes.filter(item => item.parent === column);
    const bottom = Math.max(64, ...children.map(item => item.position.y + item.node.offsetHeight));
    const right = Math.max(188, ...children.map(item => item.position.x + item.width));
    const add = column.querySelector(".config-inline-add");
    if (add) { add.style.position = "absolute"; add.style.left = "10px"; add.style.top = `${bottom + 12}px`; }
    column.style.height = `${Math.max(250, bottom + (add ? 58 : 24))}px`;
    column.style.width = `${right + 12}px`;
  }
  const roots = ciLayout.nodes.filter(item => item.parent === graph);
  let width = Math.max(1, ...roots.map(item => item.position.x + item.node.offsetWidth + 40));
  const addStage = graph.querySelector(".config-stage-add");
  if (addStage) { addStage.style.position = "absolute"; addStage.style.left = `${width}px`; addStage.style.top = "28px"; width += addStage.offsetWidth + 28; }
  graph.style.width = `${width}px`;
  graph.style.height = `${Math.max(380, ...roots.map(item => item.position.y + item.node.offsetHeight + 40))}px`;
  redrawCiEdges();
}

document.addEventListener("DOMContentLoaded", () => {
  const graph = elements["repository-config-stage-graph"];
  const movable = event => ciLayout.session?.ready && !state.repositoryConfigSaving && !ciLayout.session.loading && !ciLayout.session.resetting
    ? ciLayout.nodes.find(item => item.handle === event.target.closest(".ci-layout-handle")) : null;
  graph.addEventListener("pointerdown", event => {
    const item = movable(event);
    if (!item || event.button !== 0) return;
    ciLayout.drag = { item, session: ciLayout.session, pointer: event.pointerId, x: event.clientX, y: event.clientY,
      start: { ...item.position }, zoom: ciEditor.zoom, moved: false };
  });
  graph.addEventListener("pointermove", event => {
    const drag = ciLayout.drag;
    if (!drag || event.pointerId !== drag.pointer) return;
    const dx = event.clientX - drag.x, dy = event.clientY - drag.y;
    if (!drag.moved && Math.hypot(dx, dy) < 4) return;
    drag.moved = true; graph.setPointerCapture(event.pointerId); event.preventDefault();
    drag.item.position = clampCiNode({ x: drag.start.x + dx / drag.zoom, y: drag.start.y + dy / drag.zoom }, drag.item.minY);
    paintCiLayout();
  });
  const stop = (event, save) => {
    const drag = ciLayout.drag;
    if (!drag || event.pointerId !== drag.pointer) return;
    ciLayout.drag = null;
    if (drag.moved) {
      ciLayout.suppressClick = true; setTimeout(() => { ciLayout.suppressClick = false; }, 0);
      if (save) drag.session.move(drag.item.key, drag.item.position);
      else drag.item.position = drag.start;
      paintCiLayout();
    }
    if (graph.hasPointerCapture(event.pointerId)) graph.releasePointerCapture(event.pointerId);
  };
  graph.addEventListener("pointerup", event => stop(event, true));
  graph.addEventListener("pointercancel", event => stop(event, false));
  graph.addEventListener("lostpointercapture", event => stop(event, false));
  graph.addEventListener("click", event => { if (ciLayout.suppressClick) { event.preventDefault(); event.stopImmediatePropagation(); } }, true);
  graph.addEventListener("keydown", event => {
    const item = movable(event), delta = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }[event.key];
    if (!item || !event.altKey || event.ctrlKey || event.metaKey || !delta) return;
    event.preventDefault();
    const step = event.shiftKey ? 40 : 10;
    ciLayout.session.move(item.key, clampCiNode({ x: item.position.x + delta[0] * step, y: item.position.y + delta[1] * step }, item.minY));
    paintCiLayout();
  });
  const resetLayout = async session => {
    if (session && await session.reset() && ciLayout.session === session) renderRepositoryConfigVisual();
  };
  ciElement("ci-layout-retry").addEventListener("click", () => {
    const session = ciLayout.session;
    if (session) void (session.retryReset ? resetLayout(session) : session.ready ? session.flush() : session.load());
  });
  ciElement("ci-layout-reset").addEventListener("click", () => void resetLayout(ciLayout.session));
  window.addEventListener("beforeunload", event => {
    if ([...ciLayout.sessions.values()].some(session => session.saving || Object.keys(session.pending).length)) { event.preventDefault(); event.returnValue = ""; }
  });
});
