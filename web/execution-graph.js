/* Read-only execution inspector. Run data must never be replaced by a live route. */
const executionViews = new Map();
const executionModes = new Map();
let executionGraphRequest = 0;
let executionDetailRequest = 0;
const EXECUTION_LABELS = {
  "Inspect run status, waiting reasons, and job logs against the execution snapshot.": ["실행 스냅샷을 기준으로 진행 상태, 대기 사유와 작업 로그를 확인합니다.", "基于运行快照查看执行状态、等待原因与任务日志。"],
  "Select a job for its logs. Current configuration is separate from run history.": ["작업을 선택하면 로그를 표시합니다. 현재 설정과 실행 이력은 별도로 표시합니다.", "选择任务查看日志。当前配置与运行历史分开显示。"],
  "Run snapshot": ["실행 스냅샷", "运行快照"], "Current configuration": ["현재 설정", "当前配置"],
  "Latest run": ["최근 실행", "最近运行"], "Find failure": ["실패 위치", "定位失败"],
  "Run current configuration": ["현재 설정으로 실행", "运行当前配置"],
  "Follow running job": ["실행 중 작업 따라가기", "跟随运行任务"],
  "Collapse completed stages": ["완료 단계 접기", "折叠已完成阶段"],
  "Fit graph": ["화면 맞춤", "适应画布"], "Zoom in": ["확대", "放大"], "Zoom out": ["축소", "缩小"],
  "Reset zoom": ["원래 크기", "重置缩放"], "Job details": ["작업 상세", "任务详情"],
  "Exit code": ["종료 코드", "退出码"], "Not recorded": ["기록 없음", "无记录"],
  "Snapshot unavailable · showing recorded jobs only": ["스냅샷 없음 · 기록된 작업만 표시", "无快照 · 仅显示已记录任务"],
  "Recorded jobs differ from the snapshot · showing actual jobs": ["스냅샷과 등록된 작업이 다릅니다 · 실제 작업 기준 표시", "记录任务与快照不同 · 显示实际任务"],
  "Stages run left to right; jobs run sequentially. Select a job for logs.": ["단계는 왼쪽부터, 작업은 순차 실행됩니다. 작업을 선택하면 로그를 표시합니다.", "阶段从左到右，任务依次执行。选择任务查看日志。"],
  "Waiting for prerequisite pipelines": ["선행 파이프라인 완료 대기", "等待前置流水线"],
  "Waiting for Runner assignment": ["Runner 할당 대기", "等待分配 Runner"],
  "Waiting for a previous job": ["이전 작업 완료 대기", "等待前序任务"],
  "Waiting reason unavailable": ["대기 사유 확인 중", "等待原因尚未确认"],
  "Cancel requested": ["취소 요청됨", "已请求取消"],
  "Job not registered yet": ["아직 등록되지 않은 작업", "任务尚未注册"],
  "No recorded jobs": ["등록된 작업 없음", "无已记录任务"],
  "No job logs yet": ["아직 작업 로그가 없습니다", "暂无任务日志"],
  "Loading job logs…": ["작업 로그 불러오는 중…", "正在加载任务日志…"],
  "Load more logs": ["다음 로그 불러오기", "加载更多日志"],
  "Retry logs": ["로그 다시 불러오기", "重试日志"],
  "Showing the most recently loaded log lines": ["불러온 로그 중 최근 내용 표시", "显示最近加载的日志"],
  "Configuration only · no execution status": ["설정 미리보기 · 실행 상태 아님", "仅配置预览 · 非运行状态"],
  "Run revision": ["실행 Revision", "运行版本"], "Configuration revision": ["설정 Revision", "配置版本"],
};
for (const [key, [ko, zh]] of Object.entries(EXECUTION_LABELS)) {
  I18N.en[key] = key; I18N.ko[key] = ko; I18N["zh-CN"][key] = zh;
}

function executionRouteDetail(route, detail) {
  return { ...detail, route }; // Preserve the run's graph, OS, paths, branch and category.
}

function executionConfigured(route) {
  return { graph: route.graph, jobs: [], pipeline: { ...route, id: route.revision, status: "configured", worker_id: null, error: null, cancel_requested: false, started_at: null, finished_at: null, sparse_view_name: route.graph?.sparse_view, pipeline_needs: route.graph?.pipeline_needs } };
}

function executionJobs(detail) {
  if (detail.jobs?.length && detail.pipeline.status !== "configured") return [...detail.jobs].sort((a, b) => a.position - b.position);
  return (detail.graph?.stages || []).flatMap((stage, si) => (stage.jobs || []).map((name, ji) => ({
    id: `planned:${si}:${ji}`, name, stage: stage.name, planned: true,
    status: detail.pipeline.status === "configured" ? "configured" : "unregistered",
  })));
}

function executionStageStatus(jobs) {
  for (const status of ["failed", "running", "canceled", "queued", "unregistered"]) if (jobs.some(job => job.status === status)) return status;
  if (jobs.every(job => job.status === "skipped")) return "skipped";
  if (jobs.every(job => ["succeeded", "skipped"].includes(job.status))) return "succeeded";
  return "configured";
}

function executionWaitReason(detail, job, jobs) {
  if (detail.pipeline.cancel_requested && ["queued", "running"].includes(detail.pipeline.status)) return "Cancel requested";
  if (detail.pipeline.status === "queued") return ({ dependencies: "Waiting for prerequisite pipelines", runner: "Waiting for Runner assignment", canceling: "Cancel requested" })[detail.queue_reason] || "Waiting reason unavailable";
  if (job?.status !== "queued") return "";
  if (detail.pipeline.status !== "running") return "Waiting reason unavailable";
  const previous = jobs.slice(0, jobs.findIndex(item => item.id === job.id));
  return previous.some(item => ["running", "queued"].includes(item.status)) ? "Waiting for a previous job" : "Waiting reason unavailable";
}

function executionSnapshotMismatch(detail) {
  if (!detail.graph || !detail.jobs?.length || detail.pipeline.status === "configured") return false;
  const planned = (detail.graph.stages || []).flatMap(stage => (stage.jobs || []).map(name => [stage.name, name]));
  const actual = detail.jobs.map(job => [job.stage, job.name]);
  const normalized = values => JSON.stringify(values.map(value => JSON.stringify(value)).sort());
  return normalized(planned) !== normalized(actual);
}

function captureExecutionFocus(root) {
  const active = document.activeElement;
  if (!root.contains(active)) return;
  const workspace = active.closest(".run-workspace");
  const memo = workspace && executionViews.get(workspace.dataset.viewKey);
  if (memo) memo.focus = active.dataset.control || null;
}

function executionMemo(key) {
  if (!executionViews.has(key)) {
    // Bound retained UI/log state when browsing a long run history.
    if (executionViews.size >= 80) executionViews.delete(executionViews.keys().next().value);
    executionViews.set(key, { zoom: 1, x: 0, y: 0, selected: null, follow: false, collapse: false, expanded: new Set(), logs: null });
  }
  return executionViews.get(key);
}

function renderExecutionWorkspace(host, original, baseKey) {
  captureExecutionFocus(host);
  const mode = original.route && executionModes.get(baseKey) === "config" ? "config" : "run";
  const detail = mode === "config" ? executionConfigured(original.route) : original;
  const pipeline = detail.pipeline, configured = pipeline.status === "configured";
  const key = JSON.stringify([baseKey, mode, pipeline.id]), memo = executionMemo(key);
  const jobs = executionJobs(detail);
  if (!jobs.some(job => job.id === memo.selected)) memo.selected = (jobs.find(job => job.status === "failed") || jobs.find(job => job.status === "running") || jobs[0])?.id;
  const running = jobs.find(job => job.status === "running");
  const followChanged = memo.follow && running && running.id !== memo.selected;
  if (followChanged) memo.selected = running.id;
  const selected = jobs.find(job => job.id === memo.selected);
  if (followChanged) memo.expanded.add(selected.stage);
  host.classList.add("run-host");
  host.replaceChildren();
  const workspace = document.createElement("section"); workspace.className = "run-workspace"; workspace.dataset.viewKey = key;
  const toolbar = document.createElement("div"); toolbar.className = "run-toolbar";
  function button(label, control, action, parent = toolbar) {
    const node = document.createElement("button"); node.type = "button"; node.textContent = t(label); node.dataset.control = control;
    node.addEventListener("click", action); parent.append(node); return node;
  }
  const rerender = () => renderExecutionWorkspace(host, original, baseKey);
  if (original.route) {
    const setMode = value => {
      if (executionModes.size >= 80 && !executionModes.has(baseKey)) executionModes.delete(executionModes.keys().next().value);
      executionModes.set(baseKey, value); rerender();
    };
    const run = button("Latest run", "run", () => setMode("run"));
    run.disabled = !original.route.latest_pipeline_id; run.setAttribute("aria-pressed", String(mode === "run" && !configured));
    button("Current configuration", "config", () => setMode("config")).setAttribute("aria-pressed", String(mode === "config" || configured));
  }
  const center = document.createElement("div"); center.className = "run-center";
  const viewport = document.createElement("div"); viewport.className = "run-viewport"; viewport.tabIndex = 0; viewport.dataset.control = "viewport"; viewport.setAttribute("aria-label", t("Execution graph"));
  const canvas = document.createElement("div"); canvas.className = "run-canvas"; canvas.style.zoom = String(memo.zoom);
  const cards = new Map();
  function reveal(id, focus = false) {
    const card = cards.get(id); if (!card) return;
    const c = card.getBoundingClientRect(), v = viewport.getBoundingClientRect();
    viewport.scrollLeft += c.left - v.left - (viewport.clientWidth - c.width) / 2;
    viewport.scrollTop += c.top - v.top - (viewport.clientHeight - c.height) / 2;
    if (focus) card.focus({ preventScroll: true });
  }
  function zoom(value) {
    const old = memo.zoom, x = (viewport.scrollLeft + viewport.clientWidth / 2) / old, y = (viewport.scrollTop + viewport.clientHeight / 2) / old;
    memo.zoom = Math.max(.05, Math.min(2, value)); canvas.style.zoom = String(memo.zoom);
    reset.textContent = `${Math.round(memo.zoom * 100)}%`;
    viewport.scrollLeft = x * memo.zoom - viewport.clientWidth / 2; viewport.scrollTop = y * memo.zoom - viewport.clientHeight / 2;
    memo.x = viewport.scrollLeft; memo.y = viewport.scrollTop;
  }
  const failed = button("Find failure", "failure", () => {
    const target = jobs.find(job => job.status === "failed");
    if (target) { memo.selected = target.id; memo.follow = false; memo.expanded.add(target.stage); memo.reveal = true; rerender(); }
  });
  failed.disabled = !jobs.some(job => job.status === "failed");
  button("Follow running job", "follow", () => { memo.follow = !memo.follow; memo.reveal = memo.follow; rerender(); }).setAttribute("aria-pressed", String(memo.follow));
  button("Collapse completed stages", "collapse", () => { memo.collapse = !memo.collapse; memo.expanded.clear(); rerender(); }).setAttribute("aria-pressed", String(memo.collapse));
  button("Zoom out", "out", () => zoom(memo.zoom - .25));
  const reset = button("Reset zoom", "reset", () => zoom(1)); reset.setAttribute("aria-label", t("Reset zoom")); reset.textContent = `${Math.round(memo.zoom * 100)}%`;
  button("Zoom in", "in", () => zoom(memo.zoom + .25));
  button("Fit graph", "fit", () => {
    // Zoom can change text wrapping (especially on narrow screens). Measure
    // rendered stage bounds again after each reduction, not stale layout units
    // or the canvas's zoom-dependent min-width:100% whitespace.
    zoom(1);
    for (let pass = 0; pass < 6; pass++) {
      const origin = canvas.getBoundingClientRect();
      const bounds = [...canvas.children].map(node => node.getBoundingClientRect());
      const width = Math.max(1, ...bounds.map(rect => rect.right - origin.left + 20 * memo.zoom));
      const height = Math.max(1, ...bounds.map(rect => rect.bottom - origin.top + 20 * memo.zoom));
      const ratio = Math.min(1, (viewport.clientWidth - 8) / width, (viewport.clientHeight - 8) / height);
      if (ratio >= .999 || memo.zoom <= .05) break;
      zoom(memo.zoom * ratio);
    }
    viewport.scrollLeft = 0; viewport.scrollTop = 0; memo.x = 0; memo.y = 0;
  });
  const context = document.createElement("div"); context.className = "run-context";
  context.append(textNode(t(configured ? "Configuration only · no execution status" : detail.graph ? "Run snapshot" : "Snapshot unavailable · showing recorded jobs only"), "run-source"));
  context.append(textNode(`${t(configured ? "Configuration revision" : "Run revision")}: ${pipeline.revision || "—"}`, "run-revision"));
  const assigned = state.runners.find(runner => runner.id === pipeline.worker_id);
  context.append(textNode(`${t("Runner")}: ${assigned?.name || pipeline.worker_id || pipeline.runner_os || "—"} · ${t("Working directory")}: ${pipeline.working_directory || "."} · ${t("Sparse View")}: ${pipeline.sparse_view_name || "—"}`));
  const paths = pipeline.trigger_patterns || [];
  if (paths.length) context.append(textNode(`${t("Change paths")}: ${paths.join(", ")}`));
  const upstream = pipeline.pipeline_needs || detail.graph?.pipeline_needs || [];
  if (upstream.length) context.append(textNode(`${t("Pipeline dependencies")}: ${upstream.join(", ")}`));
  if (executionSnapshotMismatch(detail)) context.append(textNode(t("Recorded jobs differ from the snapshot · showing actual jobs"), "run-warning"));
  const reason = executionWaitReason(detail, null, jobs);
  if (reason) context.append(textNode(t(reason), "run-warning"));
  if (pipeline.error) context.append(textNode(`${t("Failure reason")}: ${pipeline.error}`, "run-error"));
  const stages = [...new Set(jobs.map(job => job.stage))];
  for (const stage of stages) {
    const stageJobs = jobs.filter(job => job.stage === stage), status = executionStageStatus(stageJobs);
    const column = document.createElement("section"); column.className = "run-stage";
    const folded = memo.collapse && ["succeeded", "skipped"].includes(status) && !memo.expanded.has(stage);
    const heading = button(`${stage}`, `stage:${stage}`, () => { memo.expanded.has(stage) ? memo.expanded.delete(stage) : memo.expanded.add(stage); rerender(); }, column);
    heading.textContent = `${stage} · ${stageJobs.length} · ${status === "unregistered" ? t("Job not registered yet") : statusLabel(status)}`;
    heading.className = "run-stage-heading";
    heading.disabled = !memo.collapse || !["succeeded", "skipped"].includes(status);
    heading.setAttribute("aria-expanded", String(!folded));
    if (!folded) for (const job of stageJobs) {
      const card = button(job.name, `job:${job.id}`, () => { memo.selected = job.id; memo.follow = false; rerender(); }, column);
      card.replaceChildren(); card.className = `run-job execution-node--${job.status}`; card.dataset.jobId = job.id;
      card.setAttribute("aria-pressed", String(job.id === memo.selected));
      card.append(textNode(job.name, "run-job-name"));
      card.append(textNode(job.status === "unregistered" ? t("Job not registered yet") : statusLabel(job.status)));
      card.append(textNode(duration(job.started_at, job.finished_at)));
      if (job.exit_code != null) card.append(textNode(`${t("Exit code")}: ${job.exit_code}`));
      cards.set(job.id, card);
    }
    canvas.append(column);
  }
  if (!jobs.length) canvas.append(textNode(t("No recorded jobs")));
  viewport.append(canvas); center.append(textNode(t("Stages run left to right; jobs run sequentially. Select a job for logs."), "run-legend"), viewport);
  const inspector = document.createElement("aside"); inspector.className = "run-inspector"; inspector.setAttribute("aria-label", t("Job details"));
  inspector.append(textNode(selected?.name || t("Job details"), "run-job-name"));
  if (selected) {
    inspector.append(textNode(selected.status === "unregistered" ? t("Job not registered yet") : statusLabel(selected.status)));
    inspector.append(textNode(`${t("Duration")}: ${duration(selected.started_at, selected.finished_at)} · ${t("Exit code")}: ${selected.exit_code ?? t("Not recorded")}`));
    const wait = executionWaitReason(detail, selected, jobs); if (wait) inspector.append(textNode(t(wait), "run-warning"));
    // Snapshot dependencies describe configuration, not an invented runtime wait reason.
    const needs = detail.graph?.dependencies?.find(edge => edge.job === selected.name)?.needs || [];
    if (needs.length && !executionSnapshotMismatch(detail)) inspector.append(textNode(`needs: ${needs.join(", ")}`));
    const log = document.createElement("pre"); log.className = "run-log"; log.tabIndex = 0; log.dataset.control = "logs"; log.setAttribute("aria-label", t("Job log"));
    inspector.append(log);
    if (selected.planned || configured) log.textContent = t(configured ? "Configuration only · no execution status" : "Job not registered yet");
    else {
      if (memo.logs?.job !== selected.id) memo.logs = { job: selected.id, rows: [], after: 0, busy: false, more: false, error: null, scroll: 0, bottom: true, trimmed: false };
      const cache = memo.logs;
      const more = button("Load more logs", "more-logs", () => void fetchLogs(), inspector);
      const note = textNode("", "run-log-note"); inspector.append(note);
      function paintLogs() {
        if (!host.isConnected || memo.logs !== cache) return;
        const scroll = cache.scroll;
        log.textContent = cache.rows.map(row => `[${row.stream}] ${row.content}`).join("") || t(cache.busy ? "Loading job logs…" : "No job logs yet");
        note.textContent = cache.error || (cache.trimmed ? t("Showing the most recently loaded log lines") : "");
        more.hidden = !cache.more && !cache.error; more.disabled = cache.busy;
        more.textContent = t(cache.error ? "Retry logs" : "Load more logs");
        log.scrollTop = cache.bottom ? log.scrollHeight : scroll;
      }
      log.addEventListener("scroll", () => { cache.scroll = log.scrollTop; cache.bottom = log.scrollTop + log.clientHeight >= log.scrollHeight - 8; });
      async function fetchLogs() {
        if (cache.busy || memo.logs !== cache || !workspace.isConnected || !workspace.getClientRects().length) return;
        cache.busy = true; cache.error = null; paintLogs();
        try {
          const rows = await api(`/api/v1/pipelines/${encodeURIComponent(pipeline.id)}/logs?job_id=${encodeURIComponent(selected.id)}&after=${cache.after}&limit=500`);
          cache.after = rows.at(-1)?.id ?? cache.after; cache.more = rows.length === 500;
          cache.rows.push(...rows);
          let size = cache.rows.reduce((sum, row) => sum + row.content.length, 0);
          while (cache.rows.length > 1 && (cache.rows.length > 2000 || size > 512000)) { size -= cache.rows.shift().content.length; cache.trimmed = true; }
        } catch (error) { cache.error = error.message; }
        finally { cache.busy = false; paintLogs(); }
      }
      window.requestAnimationFrame(() => { paintLogs(); void fetchLogs(); });
    }
  }
  const body = document.createElement("div"); body.className = "run-body"; body.append(center, inspector);
  workspace.append(toolbar, context);
  if (!configured) workspace.append(renderExecutionAnalysis(workspace, detail, memo, id => {
    const job = jobs.find(job => job.id === id);
    if (job) { memo.selected = job.id; memo.follow = false; memo.expanded.add(job.stage); memo.reveal = true; rerender(); }
  }));
  workspace.append(body); host.append(workspace);
  const savedX = memo.x, savedY = memo.y;
  viewport.addEventListener("scroll", () => { memo.x = viewport.scrollLeft; memo.y = viewport.scrollTop; });
  window.requestAnimationFrame(() => {
    if (!workspace.isConnected) return;
    viewport.scrollLeft = savedX; viewport.scrollTop = savedY;
    if (memo.reveal || followChanged) { reveal(memo.selected); memo.reveal = false; }
    if (memo.focus) {
      [...workspace.querySelectorAll("[data-control]")].find(node => node.dataset.control === memo.focus)?.focus({ preventScroll: true }); memo.focus = null;
    }
  });
}
