/* CI graph and read-only analysis. Loaded after app.js, before DOMContentLoaded. */
const ciVisual = { scope: "overview", key: null, result: null, status: "idle", error: "", timer: null, controller: null, request: 0 };
const ciElement = id => document.getElementById(id);

Object.assign(I18N.ko, {
  "Configuration check": "설정 검사", "Preview changed paths": "변경 경로 미리보기",
  "Changed files · one repository-relative path per line": "변경 파일 · 저장소 기준 상대 경로를 한 줄에 하나씩 입력",
  "Path matching only. Actual runs also depend on branch policy and Runner availability. Dependencies do not automatically trigger unmatched pipelines.": "경로 일치 여부만 검사합니다. 실제 실행은 branch 정책과 Runner 상태에도 영향을 받습니다. 의존성이 있어도 경로가 맞지 않는 파이프라인은 자동 실행되지 않습니다.",
  "All pipelines": "전체 파이프라인", "Stages & jobs": "단계·작업", "Graph view": "그래프 보기",
  "ci.checking": "검사 중…", "ci.valid": "유효한 설정", "ci.invalid": "오류 1개 · 첫 오류부터 표시", "ci.unavailable": "검사 실패 · 다시 시도",
  "ci.enterPaths": "변경 파일 경로를 입력하면 실행 대상과 일치 규칙을 표시합니다.",
  "ci.previewInvalid": "설정 오류를 수정하면 경로 미리보기를 볼 수 있습니다.",
  "ci.manual": "수동 파이프라인 · 변경 경로에 의한 자동 실행 없음",
  "ci.matched": "경로 일치", "ci.dependency": "의존성만 있음 · 자동 실행 안 됨", "ci.unmatched": "경로 불일치",
  "ci.summary": "{total}개 중 {count}개 경로 일치", "ci.rules": "일치 규칙", "ci.files": "일치 파일", "ci.referenced": "참조한 파이프라인",
  "ci.pipelineLegend": "선행 → 후행 의존성 · 카드를 선택해 설정 확인 · ‘단계·작업’에서 상세 보기",
  "ci.jobLegend": "단계는 왼쪽부터, 작업은 의존성 순서대로 순차 실행됩니다. 연결선은 needs 관계입니다.",
  "ci.layer": "의존성 단계 {number}", "ci.cycle": "순환 의존성 또는 그 영향 · 설정 검사 확인",
  "ci.line": "{line}행", "ci.openIssue": "오류 위치로 이동", "ci.inspectorError": "이 항목의 오류", "ci.noNeeds": "선행 파이프라인 없음",
});
Object.assign(I18N.en, {
  "ci.checking": "Checking…", "ci.valid": "Valid configuration", "ci.invalid": "1 error · first issue shown", "ci.unavailable": "Check failed · retry",
  "ci.enterPaths": "Enter changed file paths to see matching pipelines and rules.", "ci.previewInvalid": "Fix the configuration error to preview paths.",
  "ci.manual": "Manual pipeline · no automatic path trigger", "ci.matched": "Path match", "ci.dependency": "Dependency only · not triggered", "ci.unmatched": "No path match",
  "ci.summary": "{count} of {total} pipelines match", "ci.rules": "Matching rules", "ci.files": "Matching files", "ci.referenced": "Referenced by",
  "ci.pipelineLegend": "Prerequisite → dependent · select a card for settings · open Stages & jobs for details",
  "ci.jobLegend": "Stages run left to right; jobs run sequentially in dependency order. Lines show needs relationships.",
  "ci.layer": "Dependency level {number}", "ci.cycle": "Cycle or affected dependency · check validation",
  "ci.line": "Line {line}", "ci.openIssue": "Go to error", "ci.inspectorError": "Issue in this item", "ci.noNeeds": "No prerequisite pipelines",
});
Object.assign(I18N["zh-CN"], {
  "Configuration check": "配置检查", "Preview changed paths": "预览变更路径", "Changed files · one repository-relative path per line": "变更文件 · 每行一个仓库相对路径",
  "Path matching only. Actual runs also depend on branch policy and Runner availability. Dependencies do not automatically trigger unmatched pipelines.": "仅检查路径匹配。实际运行还取决于分支策略和 Runner 状态。依赖关系不会自动触发路径不匹配的流水线。",
  "All pipelines": "所有流水线", "Stages & jobs": "阶段与任务", "Graph view": "图表视图",
  "ci.checking": "正在检查…", "ci.valid": "配置有效", "ci.invalid": "1 个错误 · 显示首个问题", "ci.unavailable": "检查失败 · 重试",
  "ci.enterPaths": "输入变更文件路径以查看匹配的流水线和规则。", "ci.previewInvalid": "修复配置错误后可预览路径。", "ci.manual": "手动流水线 · 不会因路径变更自动触发",
  "ci.matched": "路径匹配", "ci.dependency": "仅依赖 · 不触发", "ci.unmatched": "路径不匹配", "ci.summary": "{total} 条流水线中有 {count} 条匹配",
  "ci.rules": "匹配规则", "ci.files": "匹配文件", "ci.referenced": "引用方", "ci.pipelineLegend": "前置 → 后续依赖 · 选择卡片查看设置 · 打开阶段与任务查看详情",
  "ci.jobLegend": "阶段从左到右运行，任务按依赖顺序依次运行。连线表示 needs 关系。", "ci.layer": "依赖层级 {number}",
  "ci.cycle": "循环或受影响的依赖 · 请检查配置", "ci.line": "第 {line} 行", "ci.openIssue": "转到错误", "ci.inspectorError": "此项有问题", "ci.noNeeds": "无前置流水线",
});

document.addEventListener("DOMContentLoaded", () => {
  ciElement("repository-config-form").addEventListener("input", scheduleCiAnalysis);
  ciElement("ci-path-preview").addEventListener("toggle", () => window.requestAnimationFrame(redrawCiEdges));
  for (const scope of ["overview", "detail"]) ciElement(`ci-${scope}-button`).addEventListener("click", () => {
    ciVisual.scope = scope;
    if (scope === "overview") state.repositoryConfigSelection = { type: "pipeline", pipelineIndex: state.repositoryConfigSelection?.pipelineIndex ?? 0 };
    renderRepositoryConfigVisual();
  });
});

function ciFieldKey(label) {
  const fields = { Name: "name", Category: "category", Stage: "stage", "Runner OS": "runner_os", "Working directory": "working_directory", "Change paths": "changes", "Sparse View": "sparse_view", "Timeout (seconds)": "timeout_seconds", Script: "script" };
  return Object.entries(fields).find(([key]) => t(key) === label)?.[1] ?? "";
}

function ciAnalysisInput() {
  if (!state.repositoryConfigName || state.repositoryConfigStatus !== "ready") return null;
  const content = state.repositoryConfigEditing
    ? state.repositoryConfigMode === "visual" ? serializeCiModel(state.repositoryConfigDraft) : ciElement("repository-config-editor").value
    : state.repositoryConfigContent;
  if (content === null) return null;
  return { content, changed_paths: [...new Set(ciElement("ci-changed-paths").value.split("\n").map(path => path.trim()).filter(Boolean))] };
}

function ciAnalysisKey(input) {
  return JSON.stringify([state.repositoryConfigRequest, state.repositoryConfigName, ciElement("repository-config-branch").value, state.repositoryConfigMode, input]);
}

function scheduleCiAnalysis() {
  const input = ciAnalysisInput();
  const panel = ciElement("ci-analysis-panel");
  panel.hidden = !input;
  const key = input ? ciAnalysisKey(input) : null;
  if (key === ciVisual.key) { renderCiAnalysis(); return; }
  clearTimeout(ciVisual.timer);
  ciVisual.controller?.abort();
  ciVisual.request += 1;
  ciVisual.key = key;
  ciVisual.result = null;
  ciVisual.error = "";
  ciVisual.status = input ? "checking" : "idle";
  renderCiAnalysis();
  if (input) ciVisual.timer = window.setTimeout(() => runCiAnalysis(input, key), 350);
}

async function runCiAnalysis(input = ciAnalysisInput(), key = ciVisual.key) {
  if (!input) return;
  const request = ++ciVisual.request;
  ciVisual.controller?.abort();
  ciVisual.controller = new AbortController();
  ciVisual.status = "checking";
  renderCiAnalysis();
  try {
    const result = await api(`/api/v1/repositories/${encodeURIComponent(state.repositoryConfigName)}/ci-config/analyze`, {
      method: "POST", headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() },
      body: JSON.stringify(input), signal: ciVisual.controller.signal,
    });
    if (request !== ciVisual.request || key !== ciAnalysisKey(ciAnalysisInput())) return;
    ciVisual.result = result;
    ciVisual.status = "ready";
  } catch (error) {
    if (request !== ciVisual.request || key !== ciAnalysisKey(ciAnalysisInput())) return;
    ciVisual.status = "error";
    ciVisual.error = error.message;
  }
  renderCiAnalysis();
}

async function validateCiBeforeSave(content, name) {
  // Invalid optional preview paths must never prevent saving a valid configuration.
  const result = await api(`/api/v1/repositories/${encodeURIComponent(name)}/ci-config/analyze`, {
    method: "POST", headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() },
    body: JSON.stringify({ content, changed_paths: [] }),
  });
  if (result.valid) return true;
  clearTimeout(ciVisual.timer);
  ciVisual.controller?.abort();
  ciVisual.request += 1;
  ciVisual.result = result;
  ciVisual.status = "ready";
  renderCiAnalysis();
  // Focus after the save's finally block re-enables editing.
  window.setTimeout(() => focusCiDiagnostic(result.diagnostics[0]), 0);
  return false;
}

function setCiSavingState() {
  const form = ciElement("repository-config-form");
  if (state.repositoryConfigSaving) {
    for (const control of form.querySelectorAll("button, input, select, textarea")) {
      if (!control.disabled) { control.dataset.ciSaveDisabled = "true"; control.disabled = true; }
    }
  } else {
    for (const control of form.querySelectorAll("[data-ci-save-disabled]")) {
      control.disabled = false;
      delete control.dataset.ciSaveDisabled;
    }
  }
}

function updateCiGraphScope() {
  for (const scope of ["overview", "detail"]) ciElement(`ci-${scope}-button`).setAttribute("aria-pressed", String(ciVisual.scope === scope));
  ciElement("ci-graph-legend").textContent = t(ciVisual.scope === "overview" ? "ci.pipelineLegend" : "ci.jobLegend");
}

function renderCiOverview(entries) {
  const graph = elements["repository-config-stage-graph"];
  elements["repository-config-graph-title"].textContent = t("All pipelines");
  const remaining = new Set(entries);
  const emitted = new Set();
  let level = 1;
  while (remaining.size) {
    const eligible = [...remaining].filter(entry => (entry.pipeline.needs ?? []).every(name => emitted.has(name) || !entries.some(other => other.pipeline.name === name)));
    const cyclic = !eligible.length;
    const layer = cyclic ? [...remaining] : eligible;
    const column = document.createElement("section"); column.className = "ci-overview-column";
    const heading = document.createElement("h3"); heading.textContent = cyclic ? t("ci.cycle") : t("ci.layer", { number: level++ }); column.append(heading);
    for (const entry of layer) {
      const card = document.createElement("button"); card.type = "button"; card.className = "ci-pipeline-card"; card.dataset.pipelineIndex = entry.pipelineIndex;
      card.classList.toggle("is-active", state.repositoryConfigSelection?.pipelineIndex === entry.pipelineIndex);
      card.setAttribute("aria-pressed", String(state.repositoryConfigSelection?.pipelineIndex === entry.pipelineIndex));
      const name = document.createElement("strong"); name.textContent = entry.pipeline.name;
      const meta = document.createElement("small"); meta.textContent = entry.legacy ? t("Manual") : `${entry.pipeline.runner_os} · ${entry.pipeline.category}`;
      const stages = document.createElement("span"); stages.className = "ci-card-stages"; stages.textContent = entry.pipeline.stages.join(" → ");
      const needs = document.createElement("small"); needs.className = "ci-card-needs"; needs.textContent = entry.pipeline.needs?.length ? `← ${entry.pipeline.needs.join(", ")}` : t("ci.noNeeds");
      card.append(name, meta, stages, needs);
      card.addEventListener("click", () => {
        state.repositoryConfigSelection = { type: "pipeline", pipelineIndex: entry.pipelineIndex };
        renderRepositoryConfigVisual();
      });
      column.append(card);
      remaining.delete(entry);
    }
    for (const entry of layer) emitted.add(entry.pipeline.name);
    graph.append(column);
  }
  window.requestAnimationFrame(renderCiOverviewEdges);
}

function redrawCiEdges() {
  const model = state.repositoryConfigEditing ? state.repositoryConfigDraft : state.repositoryConfigModel;
  const selected = selectedCiPipeline(model);
  if (selected && state.repositoryConfigMode === "visual") renderConfigDependencyEdges(selected.pipeline);
}

function renderCiOverviewEdges() {
  if (ciVisual.scope !== "overview" || state.repositoryConfigMode !== "visual") return;
  const graph = elements["repository-config-stage-graph"];
  graph.querySelector(".config-dependency-layer")?.remove();
  const entries = ciPipelineEntries(state.repositoryConfigEditing ? state.repositoryConfigDraft : state.repositoryConfigModel);
  const cards = new Map([...graph.querySelectorAll(".ci-pipeline-card")].map(card => [Number(card.dataset.pipelineIndex), card]));
  if (!cards.size) return;
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg"); svg.classList.add("config-dependency-layer"); svg.setAttribute("aria-hidden", "true");
  const width = Math.max(graph.scrollWidth, graph.clientWidth), height = Math.max(graph.scrollHeight, graph.clientHeight);
  svg.setAttribute("width", width); svg.setAttribute("height", height); svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
  const defs = document.createElementNS(ns, "defs"), marker = document.createElementNS(ns, "marker"), arrow = document.createElementNS(ns, "path");
  marker.id = "ci-pipeline-arrow";
  for (const [key, value] of Object.entries({ viewBox: "0 0 10 10", refX: "9", refY: "5", markerWidth: "6", markerHeight: "6", orient: "auto-start-reverse" })) marker.setAttribute(key, value);
  arrow.setAttribute("d", "M 0 0 L 10 5 L 0 10 z"); marker.append(arrow); defs.append(marker); svg.append(defs);
  const rect = graph.getBoundingClientRect();
  for (const entry of entries) for (const dependency of entry.pipeline.needs ?? []) {
    const upstream = entries.find(candidate => candidate.pipeline.name === dependency);
    const source = cards.get(upstream?.pipelineIndex), target = cards.get(entry.pipelineIndex);
    if (!source || !target) continue;
    const a = source.getBoundingClientRect(), b = target.getBoundingClientRect();
    const sameColumn = source.parentElement === target.parentElement;
    const x1 = (a.right - rect.left) / ciEditor.zoom + graph.scrollLeft, y1 = (a.top + a.height / 2 - rect.top) / ciEditor.zoom + graph.scrollTop;
    const x2 = ((sameColumn ? b.right : b.left) - rect.left) / ciEditor.zoom + graph.scrollLeft, y2 = (b.top + b.height / 2 - rect.top) / ciEditor.zoom + graph.scrollTop;
    const middle = sameColumn ? Math.max(x1, x2) + 24 : (x1 + x2) / 2;
    const path = document.createElementNS(ns, "path"); path.setAttribute("d", `M ${x1} ${y1} C ${middle} ${y1}, ${middle} ${y2}, ${x2} ${y2}`);
    path.setAttribute("marker-end", "url(#ci-pipeline-arrow)");
    path.classList.toggle("is-active", [entry.pipelineIndex, upstream.pipelineIndex].includes(state.repositoryConfigSelection?.pipelineIndex));
    svg.append(path);
  }
  graph.prepend(svg);
}

function renderCiAnalysis() {
  const status = ciElement("ci-validation-status"), diagnostics = ciElement("ci-diagnostics");
  diagnostics.replaceChildren();
  status.textContent = t(ciVisual.status === "checking" ? "ci.checking" : ciVisual.status === "error" ? "ci.unavailable" : ciVisual.result ? ciVisual.result.valid ? "ci.valid" : "ci.invalid" : "");
  status.className = ciVisual.result?.valid === false || ciVisual.status === "error" ? "ci-status-error" : "";
  if (ciVisual.status === "error") {
    const retry = document.createElement("button"); retry.type = "button"; retry.className = "ci-diagnostic";
    retry.textContent = `${ciVisual.error} · ${t("ci.unavailable")}`;
    retry.addEventListener("click", () => runCiAnalysis()); diagnostics.append(retry);
  }
  for (const issue of ciVisual.result?.diagnostics ?? []) {
    const button = document.createElement("button"); button.type = "button"; button.className = "ci-diagnostic";
    button.textContent = `${ciDiagnosticLabel(issue)}${issue.message} → ${t("ci.openIssue")}`;
    button.addEventListener("click", () => focusCiDiagnostic(issue)); diagnostics.append(button);
  }
  const summary = ciElement("ci-preview-summary"), results = ciElement("ci-preview-results");
  const expanded = new Set([...results.querySelectorAll("details[open]")].map(row => row.dataset.pipelineName));
  results.replaceChildren();
  const input = ciAnalysisInput(), analysis = ciVisual.result;
  if (ciVisual.status === "checking") summary.textContent = t("ci.checking");
  else if (ciVisual.status === "error") summary.textContent = t("ci.unavailable");
  else if (!analysis?.valid) summary.textContent = analysis ? t("ci.previewInvalid") : t("ci.enterPaths");
  else if (analysis.manual) summary.textContent = t("ci.manual");
  else if (!input?.changed_paths.length) summary.textContent = t("ci.enterPaths");
  else {
    summary.textContent = t("ci.summary", { count: analysis.pipelines.filter(pipeline => pipeline.matched).length, total: analysis.pipelines.length });
    for (const pipeline of analysis.pipelines) {
      const row = document.createElement("details"); row.className = `ci-preview-row ${ciMatchClass(pipeline)}`;
      row.dataset.pipelineName = pipeline.name;
      row.open = expanded.has(pipeline.name);
      const heading = document.createElement("summary"); heading.textContent = `${pipeline.name} · ${ciMatchLabel(pipeline)}`; row.append(heading);
      for (const [label, values] of [["ci.rules", pipeline.matching_patterns], ["ci.files", pipeline.matching_paths], ["ci.referenced", pipeline.referenced_by]]) {
        if (!values.length) continue;
        const section = document.createElement("div"), title = document.createElement("strong"), value = document.createElement("code");
        title.textContent = t(label); value.textContent = values.join("\n"); section.append(title, value); row.append(section);
      }
      results.append(row);
    }
  }
  decorateCiAnalysis();
  window.requestAnimationFrame(redrawCiEdges);
}

function ciMatchClass(pipeline) { return pipeline.matched ? "ci-matched" : pipeline.referenced_by.length ? "ci-dependency" : "ci-unmatched"; }
function ciMatchLabel(pipeline) { return t(pipeline.matched ? "ci.matched" : pipeline.referenced_by.length ? "ci.dependency" : "ci.unmatched"); }

function ciDiagnosticLabel(issue) {
  if (issue.line) return `${t("ci.line", { line: issue.line })} · `;
  if (issue.pipeline_index === null && issue.job_index === null && issue.stage_index === null) return issue.field ? `${issue.field} · ` : "";
  const entries = ciPipelineEntries(state.repositoryConfigEditing ? state.repositoryConfigDraft : state.repositoryConfigModel);
  const entry = entries.find(entry => entry.pipelineIndex === (issue.pipeline_index ?? 0));
  if (!entry) return "";
  const parts = [entry.pipeline.name];
  if (issue.job_index !== null) parts.push(entry.pipeline.jobs[issue.job_index]?.name);
  if (issue.stage_index !== null) parts.push(entry.pipeline.stages[issue.stage_index]);
  if (issue.field) parts.push(issue.field);
  return `${parts.filter(Boolean).join(" / ")} · `;
}

function decorateCiAnalysis() {
  const visual = elements["repository-config-visual"];
  if (!visual) return;
  for (const node of visual.querySelectorAll(".ci-has-error")) { node.classList.remove("ci-has-error"); node.removeAttribute("aria-invalid"); node.removeAttribute("aria-describedby"); }
  for (const badge of visual.querySelectorAll(".ci-match-badge, .ci-inspector-error")) badge.remove();
  for (const node of visual.querySelectorAll(".ci-matched, .ci-dependency, .ci-unmatched")) node.classList.remove("ci-matched", "ci-dependency", "ci-unmatched");
  const analysis = ciVisual.result;
  if (analysis?.valid && ciAnalysisInput()?.changed_paths.length) for (const pipeline of analysis.pipelines) {
    for (const card of visual.querySelectorAll(`[data-pipeline-index="${pipeline.pipeline_index}"]`)) {
      card.classList.add(ciMatchClass(pipeline));
      const badge = document.createElement("span"); badge.className = "ci-match-badge"; badge.textContent = ciMatchLabel(pipeline); card.append(badge);
    }
  }
  for (const issue of analysis?.diagnostics ?? []) {
    const index = issue.pipeline_index ?? 0;
    if (issue.pipeline_index !== null || issue.job_index !== null || issue.stage_index !== null) {
      for (const card of visual.querySelectorAll(`[data-pipeline-index="${index}"]`)) card.classList.add("ci-has-error");
    }
    const selection = state.repositoryConfigSelection;
    if (selection?.pipelineIndex !== index) continue;
    if (issue.job_index !== null) visual.querySelector(`[data-job-index="${issue.job_index}"]`)?.classList.add("ci-has-error");
    if (issue.stage_index !== null) visual.querySelector(`[data-stage-index="${issue.stage_index}"]`)?.classList.add("ci-has-error");
    const matchingSelection = issue.job_index !== null ? selection.type === "job" && selection.jobIndex === issue.job_index
      : issue.stage_index !== null ? selection.type === "stage" && selection.stageIndex === issue.stage_index : selection.type === "pipeline";
    if (!matchingSelection) continue;
    const field = [...elements["repository-config-inspector"].querySelectorAll("[data-config-field]")].find(field => field.dataset.configField === issue.field);
    if (field) { field.classList.add("ci-has-error"); field.setAttribute("aria-invalid", "true"); field.setAttribute("aria-describedby", "ci-inspector-error"); }
    const note = document.createElement("p"); note.id = "ci-inspector-error"; note.className = "ci-inspector-error"; note.textContent = `${t("ci.inspectorError")}: ${issue.message}`;
    elements["repository-config-inspector"].prepend(note);
  }
}

function focusCiDiagnostic(issue) {
  if (!issue) return;
  const model = state.repositoryConfigEditing ? state.repositoryConfigDraft : state.repositoryConfigModel;
  if (state.repositoryConfigMode === "toml" || issue.line || !model) {
    if (state.repositoryConfigMode !== "toml") {
      if (state.repositoryConfigEditing) ciElement("repository-config-editor").value = serializeCiModel(model);
      state.repositoryConfigMode = "toml";
      renderRepositoryConfig();
    }
    const editor = ciElement(state.repositoryConfigEditing ? "repository-config-editor" : "repository-config-viewer");
    editor.focus();
    if (state.repositoryConfigEditing) {
      const lines = editor.value.split("\n"), line = Math.max(0, (issue.line ?? ciIssueLine(editor.value, issue)) - 1);
      const offset = lines.slice(0, line).reduce((sum, text) => sum + text.length + 1, 0);
      const column = [...(lines[line] ?? "")].slice(0, (issue.column ?? 1) - 1).join("").length;
      editor.setSelectionRange(offset + column, offset + (lines[line]?.length ?? 0));
      editor.scrollTop = line * (parseFloat(getComputedStyle(editor).lineHeight) || 20);
    }
    editor.scrollIntoView({ block: "nearest" });
    return;
  }
  const pipelineIndex = issue.pipeline_index ?? 0;
  state.repositoryConfigSelection = issue.job_index !== null
    ? { type: "job", pipelineIndex, jobIndex: issue.job_index }
    : issue.stage_index !== null ? { type: "stage", pipelineIndex, stageIndex: issue.stage_index }
      : { type: "pipeline", pipelineIndex };
  ciVisual.scope = issue.job_index !== null || issue.stage_index !== null ? "detail" : "overview";
  renderRepositoryConfigVisual();
  const target = elements["repository-config-inspector"].querySelector(".ci-has-error")
    ?? elements["repository-config-stage-graph"].querySelector(".ci-has-error") ?? elements["repository-config-inspector"];
  target.scrollIntoView({ block: "nearest", inline: "nearest" });
  if (target.disabled || target.tagName === "FIELDSET" || target.tagName === "DIV") target.setAttribute("tabindex", "-1");
  target.focus();
}

function ciIssueLine(source, issue) {
  let pipeline = -1, job = -1, inJob = false;
  const lines = source.split("\n");
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index].trim();
    if (/^\[\[\s*pipelines\s*\]\]/.test(line)) { pipeline += 1; job = -1; inJob = false; }
    if (/^\[\[\s*(pipelines\s*\.\s*)?jobs\s*\]\]/.test(line)) { job += 1; inJob = true; }
    const field = issue.stage_index !== null ? "stages" : issue.field;
    if ((issue.pipeline_index === null || pipeline === issue.pipeline_index)
      && (issue.job_index === null ? !inJob : job === issue.job_index)
      && field && new RegExp(`^${field}\\s*=`).test(line)) return index + 1;
  }
  return 1;
}
