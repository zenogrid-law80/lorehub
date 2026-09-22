/* Local edit history and review; no persistence until the existing Save action. */
class CiEditHistory {
  constructor(snapshot, key, limit = 100, budget = 8 * 1024 * 1024) {
    this.entries = [{ data: JSON.stringify(snapshot), key }];
    this.index = 0;
    this.limit = limit;
    this.budget = budget;
    this.group = null;
    this.time = 0;
  }
  record(snapshot, key, group = null, now = Date.now()) {
    if (this.entries[this.index].key === key) {
      this.entries[this.index].data = JSON.stringify(snapshot);
      if (!group) this.group = null;
      return;
    }
    const entry = { data: JSON.stringify(snapshot), key };
    const merge = group && group === this.group && now - this.time < 800 && this.index > 0 && this.index === this.entries.length - 1;
    this.entries.splice(this.index + 1);
    if (merge) this.entries[this.index] = entry;
    else { this.entries.push(entry); this.index += 1; }
    this.group = group;
    this.time = now;
    let size = this.entries.reduce((sum, item) => sum + (item.data.length + item.key.length) * 2, 0);
    while (this.entries.length > 2 && (this.entries.length > this.limit || size > this.budget)) {
      const first = this.entries.shift(); size -= (first.data.length + first.key.length) * 2; this.index -= 1;
    }
  }
  move(delta) {
    const next = this.index + delta;
    if (next < 0 || next >= this.entries.length) return null;
    this.index = next;
    this.group = null;
    return JSON.parse(this.entries[this.index].data);
  }
}

// Exact line reconstruction, with bounded LCS work for large replacements.
function ciLineDiff(before, after, maxCells = 1000000) {
  const a = before === "" ? [] : before.split("\n"), b = after === "" ? [] : after.split("\n");
  let start = 0, endA = a.length, endB = b.length;
  while (start < endA && start < endB && a[start] === b[start]) start += 1;
  while (endA > start && endB > start && a[endA - 1] === b[endB - 1]) { endA -= 1; endB -= 1; }
  const rows = a.slice(0, start).map(text => ({ kind: "same", text }));
  const n = endA - start, m = endB - start;
  if ((n + 1) * (m + 1) > maxCells) {
    for (let i = start; i < endA; i++) rows.push({ kind: "remove", text: a[i] });
    for (let j = start; j < endB; j++) rows.push({ kind: "add", text: b[j] });
  } else {
    const width = m + 1, lcs = new Uint32Array((n + 1) * width);
    for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) {
      lcs[i * width + j] = a[start + i] === b[start + j]
        ? 1 + lcs[(i + 1) * width + j + 1] : Math.max(lcs[(i + 1) * width + j], lcs[i * width + j + 1]);
    }
    let i = 0, j = 0;
    while (i < n || j < m) {
      if (i < n && j < m && a[start + i] === b[start + j]) { rows.push({ kind: "same", text: a[start + i++] }); j++; }
      else if (i < n && (j === m || lcs[(i + 1) * width + j] >= lcs[i * width + j + 1])) rows.push({ kind: "remove", text: a[start + i++] });
      else rows.push({ kind: "add", text: b[start + j++] });
    }
  }
  for (let i = endA; i < a.length; i++) rows.push({ kind: "same", text: a[i] });
  let oldLine = 0, newLine = 0;
  return rows.map(row => ({ ...row, oldLine: row.kind === "add" ? null : ++oldLine, newLine: row.kind === "remove" ? null : ++newLine }));
}

const ciEditor = { history: null, session: null, restoring: false, generation: 0, zoom: 1, diffTimer: null };
Object.assign(I18N.ko, {
  "Reveal selection": "선택 항목으로 이동",
  "Undo": "실행 취소", "Redo": "다시 실행", "Review changes": "변경 비교", "Configuration changes": "설정 변경 내용",
  "Graph zoom": "그래프 배율", "Zoom out": "축소", "Zoom in": "확대", "Reset zoom": "원래 크기", "Fit graph": "화면 맞춤", "Pipeline graph": "파이프라인 그래프", "Drag empty space to pan": "빈 공간을 드래그해 이동",
  "Compare the saved file with the exact content that will be saved. Visual editing may rewrite comments and formatting.": "저장된 원본과 실제 저장될 내용을 비교합니다. Visual 편집은 주석과 서식을 다시 작성할 수 있습니다.",
  "ci.noChanges": "변경 사항 없음", "ci.diffCount": "추가 {added}줄 · 삭제 {removed}줄", "ci.moreLines": "다음 500줄 보기", "ci.largeDiff": "큰 설정 파일은 원문 전체로 비교합니다.", "ci.original": "저장된 원본", "ci.draft": "저장할 내용", "ci.unchanged": "변경 없는 {count}줄",
});
Object.assign(I18N.en, {
  "ci.noChanges": "No changes", "ci.diffCount": "{added} lines added · {removed} lines removed", "ci.moreLines": "Show next 500 lines", "ci.largeDiff": "Large configuration: compare the complete source below.", "ci.original": "Saved source", "ci.draft": "Content to save", "ci.unchanged": "{count} unchanged lines",
});
Object.assign(I18N["zh-CN"], {
  "Reveal selection": "定位所选项",
  "Undo": "撤销", "Redo": "重做", "Review changes": "比较变更", "Configuration changes": "配置变更", "Graph zoom": "图表缩放", "Zoom out": "缩小", "Zoom in": "放大", "Reset zoom": "原始大小", "Fit graph": "适应画面", "Pipeline graph": "流水线图表", "Drag empty space to pan": "拖动空白区域以移动",
  "Compare the saved file with the exact content that will be saved. Visual editing may rewrite comments and formatting.": "比较已保存的原文和即将保存的内容。可视化编辑可能重写注释和格式。",
  "ci.noChanges": "无变更", "ci.diffCount": "新增 {added} 行 · 删除 {removed} 行", "ci.moreLines": "显示接下来的 500 行", "ci.largeDiff": "大型配置：在下方比较完整原文。", "ci.original": "已保存的原文", "ci.draft": "即将保存的内容", "ci.unchanged": "{count} 行未变更",
});

function ciEditSnapshot() {
  const mode = state.repositoryConfigMode;
  const source = mode === "visual" ? serializeCiModel(state.repositoryConfigDraft) : ciElement("repository-config-editor").value;
  return { snapshot: { mode, source, model: state.repositoryConfigDraft, selection: state.repositoryConfigSelection, scope: ciVisual.scope }, key: `${mode}\0${source}` };
}

function resetCiEditor() {
  ciEditor.history = null; ciEditor.session = null; ciEditor.generation += 1;
  clearTimeout(ciEditor.diffTimer);
  ciElement("ci-change-review").open = false;
}

function syncCiEditor() {
  if (!state.repositoryConfigEditing) { if (ciEditor.history) resetCiEditor(); }
  else if (!ciEditor.restoring) {
    const session = JSON.stringify([state.repositoryConfigRequest, state.repositoryConfigName, state.repositoryConfigRevision]);
    const { snapshot, key } = ciEditSnapshot();
    if (ciEditor.session !== session) {
      ciEditor.session = session; ciEditor.generation += 1;
      ciEditor.history = new CiEditHistory(snapshot, key);
    } else ciEditor.history.record(snapshot, key);
  }
  updateCiHistoryControls();
  scheduleCiDiff();
}

function recordCiEdit(group = null) {
  if (state.repositoryConfigEditing && ciEditor.history && !ciEditor.restoring && !state.repositoryConfigSaving) {
    const { snapshot, key } = ciEditSnapshot();
    ciEditor.history.record(snapshot, key, group);
  }
  updateCiHistoryControls();
  scheduleCiDiff();
}

function updateCiHistoryControls() {
  const editing = state.repositoryConfigEditing;
  for (const id of ["ci-undo", "ci-redo", "ci-review"]) ciElement(id).hidden = !editing;
  ciElement("ci-change-review").hidden = !editing;
  ciElement("ci-undo").disabled = !editing || state.repositoryConfigSaving || !ciEditor.history?.index;
  ciElement("ci-redo").disabled = !editing || state.repositoryConfigSaving || !ciEditor.history || ciEditor.history.index >= ciEditor.history.entries.length - 1;
  ciElement("ci-review").disabled = state.repositoryConfigSaving;
}

function moveCiHistory(delta) {
  if (!state.repositoryConfigEditing || state.repositoryConfigSaving) return;
  recordCiEdit();
  const snapshot = ciEditor.history?.move(delta);
  if (!snapshot) return;
  ciEditor.restoring = true; ciEditor.generation += 1;
  state.repositoryConfigMode = snapshot.mode;
  state.repositoryConfigDraft = snapshot.model;
  state.repositoryConfigSelection = snapshot.selection;
  ciVisual.scope = snapshot.scope;
  ciElement("repository-config-editor").value = snapshot.source;
  try { renderRepositoryConfig(); } finally { ciEditor.restoring = false; }
  const target = snapshot.mode === "toml" ? ciElement("repository-config-editor") : elements["repository-config-inspector"].querySelector("input, select, textarea");
  target?.focus({ preventScroll: true });
}

function scheduleCiDiff() {
  clearTimeout(ciEditor.diffTimer);
  if (state.repositoryConfigEditing && ciElement("ci-change-review").open) ciEditor.diffTimer = window.setTimeout(renderCiDiff, 120);
}

function renderCiDiff() {
  if (!state.repositoryConfigEditing) return;
  const before = state.repositoryConfigContent ?? "", after = ciEditSnapshot().snapshot.source;
  const body = ciElement("ci-diff-body"), summary = ciElement("ci-diff-summary");
  body.replaceChildren();
  if (before === after) { summary.textContent = t("ci.noChanges"); return; }
  if (before.split("\n").length + after.split("\n").length > 20000) {
    summary.textContent = t("ci.largeDiff");
    for (const [label, content] of [["ci.original", before], ["ci.draft", after]]) {
      const field = document.createElement("label"), caption = document.createElement("strong"), text = document.createElement("textarea");
      caption.textContent = t(label); text.value = content; text.readOnly = true; text.rows = 15; field.append(caption, text); body.append(field);
    }
    return;
  }
  const rows = ciLineDiff(before, after);
  summary.textContent = t("ci.diffCount", { added: rows.filter(row => row.kind === "add").length, removed: rows.filter(row => row.kind === "remove").length });
  // Keep three context lines around changes, and paginate large changed blocks.
  const visible = [];
  for (let i = 0; i < rows.length;) {
    if (rows[i].kind !== "same") { visible.push(rows[i++]); continue; }
    const start = i; while (i < rows.length && rows[i].kind === "same") i++;
    const head = start > 0 ? Math.min(3, i - start) : 0, tail = i < rows.length ? Math.min(3, i - start - head) : 0;
    visible.push(...rows.slice(start, start + head));
    if (i - start > head + tail) visible.push({ kind: "gap", text: t("ci.unchanged", { count: i - start - head - tail }) });
    visible.push(...rows.slice(i - tail, i));
  }
  const table = document.createElement("table"), header = document.createElement("thead"), heading = document.createElement("tr"), content = document.createElement("tbody");
  for (const title of [t("ci.original"), t("ci.draft"), "− / +", ".lore-ci.toml"]) { const th = document.createElement("th"); th.textContent = title; th.scope = "col"; heading.append(th); }
  header.append(heading); table.append(header, content); body.append(table);
  let shown = 0;
  const more = document.createElement("button"); more.type = "button"; more.className = "button button--ghost"; more.textContent = t("ci.moreLines");
  const append = () => {
    for (const row of visible.slice(shown, shown + 500)) {
      const tr = document.createElement("tr"); tr.className = `ci-diff-${row.kind}`;
      for (const value of [row.oldLine ?? "", row.newLine ?? "", row.kind === "add" ? "+" : row.kind === "remove" ? "−" : "", row.text]) {
        const td = document.createElement("td"); td.textContent = String(value); tr.append(td);
      }
      content.append(tr);
    }
    shown += 500; more.hidden = shown >= visible.length;
  };
  more.addEventListener("click", append); body.append(more); append();
}

function applyCiZoom() {
  elements["repository-config-stage-graph"].style.zoom = String(ciEditor.zoom);
  ciElement("ci-zoom-reset").textContent = `${Math.round(ciEditor.zoom * 100)}%`;
  ciElement("ci-zoom-out").disabled = ciEditor.zoom <= .05;
  ciElement("ci-zoom-in").disabled = ciEditor.zoom >= 2;
}

// Update copy without replacing a focused inspector. Rebuilding it on blur can
// move the page between pointer-down and pointer-up and swallow toolbar clicks.
function refreshCiGraphLabels() {
  const entries = ciPipelineEntries(state.repositoryConfigDraft);
  for (const { pipeline, pipelineIndex, legacy } of entries) {
    for (const card of elements["repository-config-visual"].querySelectorAll(`[data-pipeline-index="${pipelineIndex}"]`)) {
      card.querySelector("strong").textContent = pipeline.name;
      const meta = card.querySelector("small, span:not(.ci-match-badge)");
      if (meta) meta.textContent = legacy ? t("Manual") : `${pipeline.runner_os} · ${pipeline.category}`;
      const stages = card.querySelector(".ci-card-stages"); if (stages) stages.textContent = pipeline.stages.join(" → ");
      const needs = card.querySelector(".ci-card-needs, .config-pipeline-needs");
      if (needs) needs.textContent = pipeline.needs?.length ? `← ${pipeline.needs.join(", ")}` : t("ci.noNeeds");
    }
  }
  const selected = selectedCiPipeline(state.repositoryConfigDraft);
  if (!selected) return;
  if (ciVisual.scope === "detail") elements["repository-config-graph-title"].textContent = selected.pipeline.name;
  for (const stage of elements["repository-config-stage-graph"].querySelectorAll("[data-stage-index]")) stage.querySelector("strong").textContent = selected.pipeline.stages[Number(stage.dataset.stageIndex)];
  for (const card of elements["repository-config-stage-graph"].querySelectorAll("[data-job-index]")) {
    const job = selected.pipeline.jobs[Number(card.dataset.jobIndex)];
    if (!job) continue;
    card.dataset.jobName = job.name; card.querySelector("strong").textContent = job.name;
    card.querySelector("small").textContent = t("dynamic.jobSteps", { count: job.script.length, seconds: job.timeout_seconds });
    const needs = card.querySelector(".config-job-needs"); if (needs) needs.textContent = t("dynamic.jobNeeds", { jobs: (job.needs ?? []).join(", ") });
  }
  filterCiPipelineList();
  window.requestAnimationFrame(redrawCiEdges);
}

function setCiZoom(zoom) {
  const viewport = ciElement("ci-graph-viewport"), previous = ciEditor.zoom;
  const centerX = (viewport.scrollLeft + viewport.clientWidth / 2) / previous;
  const centerY = (viewport.scrollTop + viewport.clientHeight / 2) / previous;
  ciEditor.zoom = Math.max(.05, Math.min(2, zoom));
  applyCiZoom(); redrawCiEdges();
  viewport.scrollLeft = centerX * ciEditor.zoom - viewport.clientWidth / 2;
  viewport.scrollTop = centerY * ciEditor.zoom - viewport.clientHeight / 2;
}

function fitCiGraph() {
  const viewport = ciElement("ci-graph-viewport"), graph = elements["repository-config-stage-graph"];
  setCiZoom(Math.min(1, viewport.clientWidth / Math.max(1, graph.scrollWidth), viewport.clientHeight / Math.max(1, graph.scrollHeight)));
  viewport.scrollLeft = 0; viewport.scrollTop = 0;
}

function ciGraphSelectionTarget() {
  if (state.repositoryConfigMode !== "visual" || ciElement("repository-config-visual").hidden) return null;
  const graph = ciElement("repository-config-stage-graph");
  if (ciVisual.scope === "overview") return graph.querySelector(".ci-pipeline-card.is-active");
  return graph.querySelector(".config-job-card.is-active, .config-stage-header.is-active")
    ?? graph.querySelector(".config-stage-header");
}

function updateCiRevealControl() {
  ciElement("ci-reveal-selection").disabled = state.repositoryConfigSaving || !ciGraphSelectionTarget();
}

function revealCiSelection(focus = true) {
  const target = ciGraphSelectionTarget();
  if (!target) return;
  const viewport = ciElement("ci-graph-viewport");
  const bounds = viewport.getBoundingClientRect(), item = target.getBoundingClientRect();
  // Rectangles include CSS zoom. Scroll only the graph, preserving the page and draft.
  // Oversized cards align near their start so the item name remains visible.
  viewport.scrollLeft += item.left + Math.min(item.width, Math.max(1, viewport.clientWidth - 32)) / 2
    - bounds.left - viewport.clientLeft - viewport.clientWidth / 2;
  viewport.scrollTop += item.top + Math.min(item.height, Math.max(1, viewport.clientHeight - 32)) / 2
    - bounds.top - viewport.clientTop - viewport.clientHeight / 2;
  if (focus) target.focus({ preventScroll: true });
}

document.addEventListener("DOMContentLoaded", () => {
  const form = ciElement("repository-config-form");
  form.addEventListener("input", event => {
    if (event.target.id === "repository-config-editor" || event.target.closest("#repository-config-inspector")) recordCiEdit(event.target);
  });
  form.addEventListener("keydown", event => {
    if (event.isComposing || !(event.ctrlKey || event.metaKey) || event.altKey || !state.repositoryConfigEditing || state.repositoryConfigSaving) return;
    if (event.target.closest("#ci-path-preview, #ci-change-review")) return;
    const key = event.key.toLowerCase();
    if (key === "z" || (key === "y" && !event.metaKey)) { event.preventDefault(); moveCiHistory(key === "y" || event.shiftKey ? 1 : -1); }
  });
  ciElement("ci-undo").addEventListener("click", () => moveCiHistory(-1));
  ciElement("ci-redo").addEventListener("click", () => moveCiHistory(1));
  ciElement("ci-review").addEventListener("click", () => {
    ciElement("ci-change-review").open = true; renderCiDiff();
    ciElement("ci-change-review").scrollIntoView({ block: "nearest" }); ciElement("ci-diff-body").focus({ preventScroll: true });
  });
  ciElement("ci-change-review").addEventListener("toggle", scheduleCiDiff);
  ciElement("ci-zoom-in").addEventListener("click", () => setCiZoom(Math.round((ciEditor.zoom + .25) * 100) / 100));
  ciElement("ci-zoom-out").addEventListener("click", () => setCiZoom(Math.round((ciEditor.zoom - .25) * 100) / 100));
  ciElement("ci-zoom-reset").addEventListener("click", () => setCiZoom(1));
  ciElement("ci-zoom-fit").addEventListener("click", fitCiGraph);
  ciElement("ci-reveal-selection").addEventListener("click", () => revealCiSelection());
  const viewport = ciElement("ci-graph-viewport"); let drag = null;
  viewport.addEventListener("pointerdown", event => {
    if (event.button !== 0 || event.pointerType !== "mouse" || event.target.closest("button, input, textarea, select, a")) return;
    drag = { x: event.clientX, y: event.clientY, left: viewport.scrollLeft, top: viewport.scrollTop };
    viewport.setPointerCapture(event.pointerId); viewport.classList.add("is-panning"); event.preventDefault();
  });
  viewport.addEventListener("pointermove", event => {
    if (!drag) return;
    viewport.scrollLeft = drag.left - event.clientX + drag.x; viewport.scrollTop = drag.top - event.clientY + drag.y;
  });
  const stop = () => { drag = null; viewport.classList.remove("is-panning"); };
  viewport.addEventListener("pointerup", stop); viewport.addEventListener("pointercancel", stop); viewport.addEventListener("lostpointercapture", stop);
});
