"use strict";

// Execution drawer lifecycle, summary, and bounded log paging.
// Loaded as a deferred classic script alongside app.js. Shared state, DOM
// elements, API and formatting helpers come from app.js; navigation comes from
// repository-context.js and graph rendering from execution-graph.js.
let executionDetailRequest = 0;
let executionDetailSession = 0;
const pendingPipelineCancellations = new Set();

function bindPipelineDetailEvents() {
  document.querySelectorAll(".drawer-close").forEach((button) => button.addEventListener("click", () => elements["pipeline-detail-dialog"].close()));
  elements["pipeline-detail-dialog"].addEventListener("close", () => {
    if (elements["pipeline-detail-dialog"].open) return;
    const id = state.selectedId;
    executionDetailRequest++;
    state.selectedId = null;
    state.detailLogView = null;
    if (id) closePipelineLocation(id);
  });
  elements["detail-copy-link"].addEventListener("click", async () => {
    if (!state.selectedId) return;
    try { await navigator.clipboard.writeText(pipelinePermalink(state.selectedId)); toast(t("dynamic.urlCopied"), "success"); }
    catch (_) { toast(t("dynamic.copyFailed"), "error"); }
  });
  elements["detail-log-more"].addEventListener("click", () => void loadPipelineLogs(state.selectedId));
  elements["detail-log-restart"].addEventListener("click", () => void restartDetailLogs());
  elements["detail-log-follow"].addEventListener("click", () => {
    const cache = state.detailLogView;
    if (!cache) return;
    cache.follow = !cache.follow;
    renderLogs(cache.rows, cache.prunedAt);
    if (cache.follow && !cache.more) void loadPipelineLogs(cache.id);
  });
  elements["pipeline-log"].addEventListener("scroll", () => {
    const cache = state.detailLogView, terminal = elements["pipeline-log"];
    if (!cache?.follow || terminal.scrollTop + terminal.clientHeight >= terminal.scrollHeight - 8) return;
    cache.follow = false;
    renderLogs(cache.rows, cache.prunedAt);
  });
  elements["detail-retry"].addEventListener("click", () => { if (state.selectedId) void openPipeline(state.selectedId); });
  elements["cancel-pipeline-button"].addEventListener("click", cancelPipeline);
}

const detailLogCopy = {
  more: ["로그 500건 더 보기", "Load 500 more records", "再加载 500 条记录"],
  retry: ["로그 다시 시도", "Retry logs", "重试日志"],
  follow: ["새 출력 따라가기", "Follow new output", "跟随新输出"],
  restart: ["처음부터 보기", "Start from the beginning", "从头查看"],
  detailRetry: ["상세 다시 시도", "Retry detail", "重试详情"],
  copyLink: ["실행 링크 복사", "Copy run link", "复制运行链接"],
  openLink: ["실행 링크 열기", "Open run link", "打开运行链接"],
  paused: ["자동 갱신과 스크롤이 일시 정지되었습니다.", "Automatic updates and scrolling are paused.", "已暂停自动更新和滚动。"],
  pending: ["뒤에 로그가 더 있을 수 있습니다. 더 보기를 눌러 계속 읽으세요.", "More records may follow. Load more to continue reading.", "后面可能还有日志，请继续加载。"],
  trimmed: ["화면에는 최근에 읽은 로그 일부만 유지합니다. 이전 부분은 ‘처음부터 보기’로 다시 읽을 수 있습니다.", "Only the most recently loaded output is kept on screen. Restart to read earlier output again.", "仅保留最近加载的输出，可从头重新查看较早的内容。"],
};
function dlt(key) { return detailLogCopy[key][state.locale === "ko" ? 0 : state.locale === "zh-CN" ? 2 : 1]; }

function resetDetailLogs(id, prunedAt = null) {
  state.detailLogView = { id, prunedAt, rows: [], after: 0, more: true, started: false, busy: false, follow: true, trimmed: false, error: null };
  return state.detailLogView;
}

async function restartDetailLogs() {
  const cache = state.detailLogView;
  if (!cache || cache.busy) return;
  Object.assign(resetDetailLogs(cache.id, cache.prunedAt), { follow: false, status: cache.status });
  elements["pipeline-log"].scrollTop = 0;
  await loadPipelineLogs(cache.id);
}

function clearPipelineDetail(message) {
  state.detailLogView = null;
  elements["detail-title"].textContent = "";
  elements["detail-repository"].textContent = "";
  elements["detail-summary"].replaceChildren();
  elements["execution-graph"].replaceChildren();
  elements["execution-graph-section"].hidden = true;
  elements["job-list"].replaceChildren(textNode(message, "job-empty"));
  elements["job-count"].textContent = "";
  elements["cancel-pipeline-button"].hidden = true;
  renderLogs([], null);
}

async function openPipeline(id, { fromLocation = false } = {}) {
  executionDetailSession++;
  if (!fromLocation) recordPipelineLocation(id);
  state.selectedId = id;
  clearPipelineDetail(t("dynamic.loading"));
  resetDetailLogs(id);
  elements["detail-retry"].hidden = true;
  if (!elements["pipeline-detail-dialog"].open) elements["pipeline-detail-dialog"].showModal();
  elements["detail-title"].textContent = t("dynamic.loading");
  renderPipelinePermalink(id);
  try { await loadPipelineDetail(id); } catch (error) { toast(error.message, "error"); }
}

function renderPipelinePermalink(id) {
  elements["detail-copy-link"].textContent = dlt("copyLink");
  elements["detail-permalink"].textContent = dlt("openLink");
  elements["detail-permalink"].href = pipelinePermalink(id);
}

async function loadPipelineDetail(id) {
  if (!id || state.selectedId !== id) return;
  const request = ++executionDetailRequest;
  let detail;
  try { detail = await api(`/api/v1/pipelines/${encodeURIComponent(id)}`); }
  catch (error) {
    if (state.selectedId !== id || request !== executionDetailRequest) return;
    clearPipelineDetail(error.message);
    elements["detail-title"].textContent = t("dynamic.requestFailed", { status: error.status || "—" });
    elements["detail-retry"].hidden = false;
    elements["detail-retry"].textContent = dlt("detailRetry");
    throw error;
  }
  if (state.selectedId !== id || request !== executionDetailRequest) return;
  renderPipelinePermalink(id);
  const pipeline = detail.pipeline;
  const prunedAt = pipeline.logs_pruned_at || null;
  if (state.detailLogView?.id !== id || state.detailLogView.prunedAt !== prunedAt) resetDetailLogs(id, prunedAt);
  elements["detail-retry"].hidden = true;
  elements["detail-repository"].textContent = [
    repositoryName(pipeline.repository_url),
    pipeline.branch,
    pipeline.category && pipelineCategory(pipeline),
  ].filter(Boolean).join(" / ");
  elements["detail-title"].textContent = pipeline.pipeline_name || revisionLabel(pipeline);
  renderDetailSummary(pipeline, detail.sparse_view_rules);
  renderExecutionGraph(pipeline, detail.jobs, detail.graph, detail.queue_reason);
  renderJobs(detail.jobs);
  const cancellable = ["queued", "running"].includes(pipeline.status) && !pipeline.cancel_requested;
  elements["cancel-pipeline-button"].hidden = !cancellable;
  elements["cancel-pipeline-button"].disabled = pendingPipelineCancellations.has(id);
  const cache = state.detailLogView;
  cache.status = pipeline.status;
  renderLogs(cache.rows, cache.prunedAt);
  // Summary and jobs are visible before the first log page resolves. A full page
  // requires an explicit next-page request instead of draining an unbounded log.
  if (!cache.started || (cache.follow && !cache.more && !cache.error)) await loadPipelineLogs(id);
}

function appendDetailLogs(cache, rows) {
  cache.after = rows.at(-1)?.id ?? cache.after;
  cache.more = rows.length === 500;
  cache.started = true;
  cache.rows.push(...rows);
  let size = cache.rows.reduce((total, row) => total + row.content.length, 0);
  while (cache.rows.length > 1 && (cache.rows.length > 2000 || size > 512000)) {
    size -= cache.rows.shift().content.length;
    cache.trimmed = true;
  }
  if (size > 512000) {
    cache.rows[0] = { ...cache.rows[0], content: cache.rows[0].content.slice(-512000) };
    cache.trimmed = true;
  }
}

async function loadPipelineLogs(id) {
  const cache = state.detailLogView;
  if (!cache || cache.id !== id || state.selectedId !== id || cache.busy) return;
  cache.busy = true;
  cache.error = null;
  renderLogs(cache.rows, cache.prunedAt);
  try {
    const rows = await api(`/api/v1/pipelines/${encodeURIComponent(id)}/logs?after=${cache.after}&limit=500`);
    if (state.detailLogView !== cache || state.selectedId !== id) return;
    appendDetailLogs(cache, rows);
  } catch (error) {
    if (state.detailLogView !== cache || state.selectedId !== id) return;
    if ([403, 404].includes(error.status)) {
      clearPipelineDetail(error.message);
      elements["detail-retry"].hidden = false;
      elements["detail-retry"].textContent = dlt("detailRetry");
      return;
    }
    cache.error = error.message;
    cache.started = true;
  } finally {
    cache.busy = false;
    if (state.detailLogView === cache && state.selectedId === id) renderLogs(cache.rows, cache.prunedAt);
  }
}

function renderExecutionGraph(pipeline, jobs, snapshot, queueReason) {
  const section = elements["execution-graph-section"];
  const graph = elements["execution-graph"];
  if (!pipeline.pipeline_name) {
    section.hidden = true;
    graph.replaceChildren();
    return;
  }

  section.hidden = false;
  renderExecutionWorkspace(graph, { pipeline, jobs, graph: snapshot, queue_reason: queueReason }, `drawer:${pipeline.id}`);
}

function renderDetailSummary(pipeline, sparseViewRules) {
  elements["detail-summary"].replaceChildren();
  const values = [
    [t("Status"), statusLabel(pipeline.status)],
    [t("Revision"), revisionLabel(pipeline)],
    [t("Created"), formatDate(pipeline.created_at)],
    [t("Duration"), duration(pipeline.started_at, pipeline.finished_at)],
  ];
  if (pipeline.branch) values.splice(2, 0, [t("Branch"), pipeline.branch]);
  if (pipeline.category) values.push([t("Category"), pipelineCategory(pipeline)]);
  if (pipeline.pipeline_name) values.push([t("Pipeline"), pipeline.pipeline_name], [t("Runner OS"), osLabel(pipeline.runner_os)]);
  values.push([t("Sparse View"), pipeline.sparse_view_name || t("dynamic.noSparseView")]);
  for (const [label, value] of values) {
    const item = document.createElement("div"); item.className = "summary-item";
    const title = document.createElement("span"); title.textContent = label;
    const content = document.createElement("strong"); content.textContent = value;
    item.append(title, content); elements["detail-summary"].append(item);
  }
  if (pipeline.error) {
    const item = document.createElement("div"); item.className = "summary-item summary-item--error";
    const title = document.createElement("span"); title.textContent = t("Failure reason");
    const content = document.createElement("strong"); content.textContent = pipeline.error;
    item.append(title, content); elements["detail-summary"].append(item);
  }
  if (pipeline.sparse_view_name && sparseViewRules) {
    const item = document.createElement("div"); item.className = "summary-item summary-item--view-rules";
    const title = document.createElement("span"); title.textContent = `${t("View rules")} · ${t("dynamic.viewSnapshot")}`;
    const content = document.createElement("code"); content.textContent = sparseViewRules.trim();
    item.append(title, content); elements["detail-summary"].append(item);
  }
}

function renderJobs(jobs) {
  elements["job-list"].replaceChildren();
  elements["job-count"].textContent = tc("dynamic.jobs", jobs.length);
  if (!jobs.length) {
    const empty = textNode(t("dynamic.workerPreparing"), "job-empty");
    elements["job-list"].append(empty);
    return;
  }
  for (const job of jobs) {
    const row = document.createElement("div"); row.className = "job-row";
    const stateIcon = textNode(job.status === "succeeded" ? "✓" : job.status === "failed" ? "!" : job.status === "running" ? "▶" : "·", `job-state job-state--${job.status}`);
    row.append(stateIcon, textNode(job.name, "job-name"), textNode(job.stage, "job-stage"), textNode(duration(job.started_at, job.finished_at), "job-duration"));
    elements["job-list"].append(row);
  }
}

function renderLogs(logs, prunedAt) {
  const terminal = elements["pipeline-log"];
  const scroll = terminal.scrollTop;
  const cache = state.detailLogView;
  // Keep existing text nodes intact so polling does not destroy selection.
  let view = terminal.logRender;
  const reset = !view || view.cache !== cache || view.prunedAt !== prunedAt;
  if (reset) {
    terminal.replaceChildren();
    view = terminal.logRender = { cache, prunedAt, nodes: new Map(), notice: null };
  }
  const notice = prunedAt ? t("dynamic.logsPruned") : !logs.length ? t(cache?.busy ? "dynamic.loadingLogs" : "dynamic.noLogs") : null;
  if (notice !== null) {
    if (!view.notice) { view.notice = textNode(notice, "terminal-muted"); terminal.prepend(view.notice); }
    else if (view.notice.textContent !== notice) view.notice.textContent = notice;
  } else if (view.notice) { view.notice.remove(); view.notice = null; }
  const retained = new Set(logs.map((log, index) => log.id ?? index));
  const oldHeight = terminal.scrollHeight;
  let removed = false, changed = reset;
  for (const [id, node] of view.nodes) {
    if (!retained.has(id)) { node.remove(); view.nodes.delete(id); removed = changed = true; }
  }
  const removedHeight = removed ? oldHeight - terminal.scrollHeight : 0;
  for (const [index, log] of logs.entries()) {
    const id = log.id ?? index;
    let line = view.nodes.get(id);
    if (!line) {
      line = document.createElement("span");
      view.nodes.set(id, line); terminal.append(line); changed = true;
    }
    const className = log.stream === "stderr" ? "terminal-stderr" : log.stream === "system" ? "terminal-system" : "";
    if (line.className !== className) line.className = className;
    if (line.textContent !== log.content) { line.textContent = log.content; changed = true; }
  }
  const followChanged = cache?.follow && !view.follow;
  if (cache?.follow && (changed || followChanged)) terminal.scrollTop = terminal.scrollHeight;
  else if (removed || reset) terminal.scrollTop = Math.max(0, scroll - removedHeight);
  view.follow = cache?.follow;
  const more = elements["detail-log-more"];
  more.hidden = !cache || (!cache.more && !cache.error);
  more.disabled = !cache || cache.busy;
  more.textContent = dlt(cache?.error ? "retry" : "more");
  const follow = elements["detail-log-follow"];
  follow.disabled = !cache;
  follow.textContent = dlt("follow");
  follow.setAttribute("aria-pressed", String(Boolean(cache?.follow)));
  elements["detail-log-restart"].disabled = !cache || cache.busy;
  elements["detail-log-restart"].textContent = dlt("restart");
  elements["detail-log-note"].textContent = cache ? [cache.error, cache.trimmed && dlt("trimmed"), !cache.follow && dlt("paused"), cache.more && dlt("pending")].filter(Boolean).join(" ") : "";
}

async function cancelPipeline() {
  const id = state.selectedId;
  if (!id || pendingPipelineCancellations.has(id)) return;
  const session = executionDetailSession;
  const isCurrentDetail = () => state.selectedId === id && executionDetailSession === session;
  // Polling and reopening a drawer must not submit a second cancellation.
  pendingPipelineCancellations.add(id);
  elements["cancel-pipeline-button"].disabled = true;
  try {
    await api(`/api/v1/pipelines/${encodeURIComponent(id)}/cancel`, { method: "POST", headers: { "X-CSRF-Token": csrfToken() } });
    if (isCurrentDetail()) toast(t("dynamic.cancelRequested"), "success");
    await loadPipelines(false);
    if (isCurrentDetail()) await loadPipelineDetail(id);
  } catch (error) {
    if (isCurrentDetail()) toast(error.message, "error");
  } finally {
    pendingPipelineCancellations.delete(id);
    // A reopened drawer for this run also needs its pending state released.
    if (state.selectedId === id) elements["cancel-pipeline-button"].disabled = false;
  }
}
