/* On-demand execution analysis; all server data is rendered as text, never HTML. */
const EXECUTION_ANALYSIS_LABELS = {
  "Execution analysis": ["실행 분석", "运行分析"], "Dependencies": ["파이프라인 연결", "流水线依赖"],
  "Timeline": ["시간 타임라인", "时间线"], "Previous run comparison": ["이전 실행 비교", "对比上次运行"],
  "Loading analysis…": ["분석 불러오는 중…", "正在加载分析…"], "Retry analysis": ["분석 다시 불러오기", "重试分析"],
  "Analysis observed at": ["분석 조회 시각", "分析观测时间"],
  "Latest runs in the same repository, branch and revision. Historical prerequisite run IDs were not recorded.": ["같은 저장소·branch·revision의 최근 실행 관계입니다. 실제 사용된 선행 실행 ID는 기록되어 있지 않습니다.", "同仓库、分支和版本中的最近运行关系。未记录历史实际使用的前置运行 ID。"],
  "Upstream": ["선행 실행", "前置运行"], "Selected run": ["선택한 실행", "选中运行"], "Downstream": ["후행 실행", "后续运行"],
  "No related runs": ["연결된 실행 없음", "无关联运行"],
  "No run in this revision · not automatically created": ["동일 revision 실행 없음 · 자동 생성 안 됨", "此版本无运行 · 不会自动创建"],
  "Showing the first 100 relationships": ["최대 100개 연결 표시", "最多显示 100 个关联"],
  "Queue wait": ["실행 전 대기", "排队等待"], "Execution time": ["실행 시간", "执行时间"],
  "Start delay": ["작업 시작 전 지연", "启动前延迟"], "Not started": ["실행되지 않음", "未启动"],
  "Job start delay includes preparation and preceding jobs; it is not an additional queue wait.": ["작업 시작 전 지연에는 준비와 이전 작업 시간이 포함됩니다. 별도의 큐 대기 시간이 아닙니다.", "任务启动延迟包括准备及前序任务时间，并非额外的排队时间。"],
  "Missing or inconsistent timestamps": ["시간 기록 없음 또는 불일치", "时间戳缺失或不一致"],
  "No previous completed run": ["비교할 이전 완료 실행 없음", "无可对比的已完成运行"],
  "Baseline: the most recent run completed before this run was submitted.": ["기준: 이 실행이 제출되기 전에 완료된 가장 최근 실행입니다.", "基准：本次运行提交之前已完成的最近一次运行。"],
  "Reference comparison, not a regression diagnosis. Source, scripts, Runner load and environment may differ.": ["참고용 시간 비교이며 성능 저하 판정이 아닙니다. 소스·스크립트·Runner 부하·환경이 다를 수 있습니다.", "仅供参考，并非性能退化诊断。源码、脚本、Runner 负载及环境可能不同。"],
  "OS differs": ["운영체제 다름", "操作系统不同"], "Working directory differs": ["작업 디렉터리 다름", "工作目录不同"],
  "Sparse View differs": ["Sparse View 다름", "Sparse View 不同"], "Job configuration differs": ["작업 구성 다름", "任务配置不同"],
  "Snapshot missing": ["비교 스냅샷 없음", "缺少对比快照"], "Revision differs": ["Revision 다름", "版本不同"], "Runner differs": ["Runner 다름", "Runner 不同"],
  "Execution conditions differ · deltas unavailable": ["실행 조건 다름 · 증감 계산 안 함", "执行条件不同 · 不计算差异"],
  "Only matching, successfully completed jobs are compared.": ["일치하는 작업 중 양쪽 모두 성공 완료된 작업만 증감을 계산합니다.", "仅对比匹配且两次均成功完成的任务。"],
  "Job": ["작업", "任务"], "Current": ["이번 실행", "本次"], "Previous": ["이전 실행", "上次"], "Change": ["증감", "变化"],
  "No matching previous job": ["일치하는 이전 작업 없음", "无匹配的上次任务"],
  "Completed successful jobs only": ["양쪽 성공 완료 필요", "需要两次成功完成"],
};
for (const [key, [ko, zh]] of Object.entries(EXECUTION_ANALYSIS_LABELS)) {
  I18N.en[key] = key; I18N.ko[key] = ko; I18N["zh-CN"][key] = zh;
}

function executionTimestamp(value) {
  if (typeof value !== "string" || !value) return null;
  const time = Date.parse(value); return Number.isFinite(time) ? time : null;
}

function executionElapsed(job) {
  const start = executionTimestamp(job?.started_at), end = executionTimestamp(job?.finished_at);
  return start !== null && end !== null && end >= start ? end - start : null;
}

function executionTimeline(pipeline, jobs, observedAt) {
  const origin = executionTimestamp(pipeline.created_at), now = executionTimestamp(observedAt);
  if (origin === null || now === null || now < origin) return null;
  const live = ["queued", "running"].includes(pipeline.status);
  const end = executionTimestamp(pipeline.finished_at) ?? (live ? now : null);
  const start = executionTimestamp(pipeline.started_at);
  if (end === null || end < origin || end > now) return null;
  const interval = (from, to) => from !== null && to !== null && from >= origin && to >= from && to <= end ? { from: from - origin, to: to - origin, ms: to - from } : null;
  const queueEnd = start ?? (["queued", "canceled", "failed"].includes(pipeline.status) ? end : null);
  const rows = [{ id: null, name: pipeline.pipeline_name || "Pipeline", status: pipeline.status,
    wait: interval(origin, queueEnd), run: interval(start, end), note: queueEnd === null ? "Missing or inconsistent timestamps" : "" }];
  for (const job of jobs) {
    const began = executionTimestamp(job.started_at), finished = executionTimestamp(job.finished_at);
    const jobEnd = finished ?? (job.status === "running" && pipeline.status === "running" ? now : null);
    const waitEnd = began ?? (job.status === "queued" && pipeline.status === "running" ? now : null);
    rows.push({ id: job.id, name: `${job.stage} / ${job.name}`, status: job.status,
      wait: interval(start, waitEnd), run: interval(began, jobEnd),
      note: began === null && ["queued", "skipped", "canceled"].includes(job.status) ? "Not started" : !interval(began, jobEnd) ? "Missing or inconsistent timestamps" : "" });
  }
  return { origin, total: Math.max(1, end - origin), rows };
}

function executionComparisons(current, previous, comparable) {
  const key = job => JSON.stringify([job.stage, job.name]);
  const before = new Map(), counts = new Map(), currentCounts = new Map();
  for (const job of current) { const k = key(job); currentCounts.set(k, (currentCounts.get(k) || 0) + 1); }
  for (const job of previous || []) { const k = key(job); before.set(k, job); counts.set(k, (counts.get(k) || 0) + 1); }
  return current.map(job => {
    const old = before.get(key(job)), currentMs = executionElapsed(job), previousMs = executionElapsed(old);
    const reason = !old || counts.get(key(job)) !== 1 || currentCounts.get(key(job)) !== 1 ? "No matching previous job"
      : !comparable ? "Execution conditions differ · deltas unavailable"
      : job.status !== "succeeded" || old.status !== "succeeded" ? "Completed successful jobs only"
      : currentMs === null || previousMs === null ? "Missing or inconsistent timestamps" : "";
    const delta = reason ? null : currentMs - previousMs;
    return { job, currentMs, previousMs, delta, percent: delta !== null && previousMs > 0 ? delta / previousMs * 100 : null, reason };
  }).sort((a, b) => (b.delta ?? -Infinity) - (a.delta ?? -Infinity));
}

function executionTimeLabel(ms) {
  return ms === null ? "—" : t("unit.second", { count: new Intl.NumberFormat(localeTag(), { maximumFractionDigits: 1 }).format(ms / 1000) });
}

function renderExecutionAnalysis(workspace, detail, memo, selectJob) {
  const disclosure = document.createElement("details"); disclosure.className = "run-analysis"; disclosure.open = !!memo.analysisOpen;
  const summary = document.createElement("summary"); summary.textContent = t("Execution analysis"); summary.dataset.control = "analysis-open";
  const content = document.createElement("div"); content.className = "run-analysis-content";
  disclosure.append(summary, content);
  memo.analysis ??= { data: null, at: 0, busy: false, error: null };
  const cache = memo.analysis;
  let retry;
  function action(label, control, handler, parent) {
    const button = document.createElement("button"); button.type = "button"; button.textContent = label; button.dataset.control = control;
    button.addEventListener("click", handler); parent.append(button); return button;
  }
  function paint(resetScroll = false) {
    if (!disclosure.isConnected || !disclosure.open) return;
    const focused = content.contains(document.activeElement) ? document.activeElement.dataset.control : null;
    const oldScroll = resetScroll ? 0 : content.querySelector(".run-analysis-scroll")?.scrollTop ?? memo.analysisScroll ?? 0;
    content.replaceChildren();
    const nav = document.createElement("div"); nav.className = "run-analysis-tabs";
    nav.setAttribute("role", "group"); nav.setAttribute("aria-label", t("Execution analysis"));
    const tab = memo.analysisTab || "dependencies";
    for (const [value, label] of [["dependencies", "Dependencies"], ["timeline", "Timeline"], ["comparison", "Previous run comparison"]]) {
      action(t(label), `analysis:${value}`, () => { memo.analysisTab = value; memo.analysisScroll = 0; paint(true); }, nav).setAttribute("aria-pressed", String(tab === value));
    }
    content.append(nav);
    if (cache.error) {
      content.append(textNode(cache.error, "run-error"));
      retry = action(t("Retry analysis"), "analysis-retry", () => void load(true), content); retry.disabled = cache.busy;
    }
    if (!cache.data) { if (!cache.error) content.append(textNode(t("Loading analysis…"))); return; }
    const data = cache.data;
    content.append(textNode(`${t("Analysis observed at")}: ${new Date(data.observed_at).toLocaleTimeString(localeTag())}`, "run-analysis-note"));
    const scroll = document.createElement("div"); scroll.className = "run-analysis-scroll"; scroll.tabIndex = 0; scroll.dataset.control = "analysis-scroll";
    scroll.addEventListener("scroll", () => { memo.analysisScroll = scroll.scrollTop; });
    if (tab === "dependencies") {
      scroll.append(textNode(t("Latest runs in the same repository, branch and revision. Historical prerequisite run IDs were not recorded."), "run-analysis-note"));
      const links = document.createElement("div"); links.className = "run-relations";
      const groups = [["Upstream", data.upstream, data.upstream_truncated], ["Selected run", [{ name: data.current.pipeline.pipeline_name || "Pipeline", run_id: data.current.pipeline.id, status: data.current.pipeline.status }], false], ["Downstream", data.downstream, data.downstream_truncated]];
      for (const [label, runs, truncated] of groups) {
        const column = document.createElement("section"); column.append(textNode(t(label), "run-job-name"));
        for (const run of runs) {
          const card = action(`${run.name} · ${run.status ? statusLabel(run.status) : t("No run in this revision · not automatically created")}`, `related:${label}:${run.name}`, () => void openPipeline(run.run_id), column);
          card.disabled = !run.run_id || run.run_id === data.current.pipeline.id;
          if (run.created_at) card.title = `${run.run_id}\n${run.created_at}`;
        }
        if (!runs.length) column.append(textNode(t("No related runs")));
        if (truncated) column.append(textNode(t("Showing the first 100 relationships"), "run-warning"));
        links.append(column);
      }
      scroll.append(links);
    } else if (tab === "timeline") {
      const timeline = executionTimeline(data.current.pipeline, data.current.jobs, data.observed_at);
      scroll.append(textNode(t("Job start delay includes preparation and preceding jobs; it is not an additional queue wait."), "run-analysis-note"));
      const legend = document.createElement("div"); legend.className = "run-time-legend";
      legend.append(textNode(t("Queue wait"), "run-time-wait-label"), textNode(t("Execution time"), "run-time-work-label")); scroll.append(legend);
      if (!timeline) scroll.append(textNode(t("Missing or inconsistent timestamps")));
      else {
        for (const row of timeline.rows) {
          const line = document.createElement("div"); line.className = "run-time-row";
          const label = action(`${row.name} · ${statusLabel(row.status)}`, `timeline:${row.id}`, () => selectJob(row.id), line); label.disabled = !row.id;
          const track = document.createElement("div"); track.className = "run-time-track"; track.setAttribute("aria-hidden", "true");
          for (const [kind, interval] of [["wait", row.wait], ["work", row.run]]) if (interval) {
            const bar = document.createElement("span"); bar.className = `run-time-${kind}`;
            bar.style.left = `${interval.from / timeline.total * 100}%`; bar.style.width = `${interval.ms / timeline.total * 100}%`; track.append(bar);
          }
          const copy = textNode(`${t(row.id ? "Start delay" : "Queue wait")}: ${executionTimeLabel(row.wait?.ms ?? null)} · ${t("Execution time")}: ${executionTimeLabel(row.run?.ms ?? null)}${row.note ? ` · ${t(row.note)}` : ""}`);
          line.append(track, copy); scroll.append(line);
        }
      }
    } else {
      scroll.append(textNode(t("Baseline: the most recent run completed before this run was submitted."), "run-analysis-note"));
      if (!data.previous) scroll.append(textNode(t("No previous completed run")));
      else {
        const previous = data.previous.pipeline;
        action(`${t("Previous")}: #${previous.revision_number} · ${statusLabel(previous.status)} · ${previous.id}`, "analysis-previous", () => void openPipeline(previous.id), scroll);
        scroll.append(textNode(t("Reference comparison, not a regression diagnosis. Source, scripts, Runner load and environment may differ."), "run-analysis-note"));
        const warnings = { runner_os: "OS differs", working_directory: "Working directory differs", sparse_view: "Sparse View differs", layout: "Job configuration differs", snapshot_missing: "Snapshot missing", revision: "Revision differs", runner: "Runner differs" };
        for (const warning of data.comparison_warnings || []) scroll.append(textNode(t(warnings[warning] || warning), "run-warning"));
        if (!data.comparable) scroll.append(textNode(t("Execution conditions differ · deltas unavailable"), "run-warning"));
        scroll.append(textNode(t("Only matching, successfully completed jobs are compared."), "run-analysis-note"));
        if (!data.current.jobs.length) scroll.append(textNode(t("No recorded jobs")));
        const table = document.createElement("table"); table.className = "run-comparison";
        const head = document.createElement("thead"), heading = document.createElement("tr");
        for (const label of ["Job", "Current", "Previous", "Change"]) { const cell = document.createElement("th"); cell.scope = "col"; cell.textContent = t(label); heading.append(cell); }
        head.append(heading); table.append(head);
        const body = document.createElement("tbody");
        for (const result of executionComparisons(data.current.jobs, data.previous.jobs, data.comparable)) {
          const row = document.createElement("tr"), name = document.createElement("td");
          action(`${result.job.stage} / ${result.job.name}`, `compare:${result.job.id}`, () => selectJob(result.job.id), name); row.append(name);
          const delta = result.delta === null ? t(result.reason) : `${result.delta > 0 ? "+" : result.delta < 0 ? "−" : ""}${executionTimeLabel(Math.abs(result.delta))}${result.percent === null ? "" : ` (${result.percent > 0 ? "+" : ""}${result.percent.toFixed(1)}%)`}`;
          for (const value of [executionTimeLabel(result.currentMs), executionTimeLabel(result.previousMs), delta]) { const cell = document.createElement("td"); cell.textContent = value; row.append(cell); }
          if (result.delta > 0) row.lastChild.className = "run-warning";
          body.append(row);
        }
        table.append(body); scroll.append(table);
      }
    }
    content.append(scroll);
    scroll.scrollTop = oldScroll;
    if (focused) [...content.querySelectorAll("[data-control]")].find(node => node.dataset.control === focused)?.focus({ preventScroll: true });
  }
  // A pending fetch can finish after a polling render; publish into the current
  // disclosure, never into a detached or different run's panel.
  cache.paint = paint;
  async function load(force = false) {
    if (!disclosure.isConnected || !disclosure.open || !disclosure.getClientRects().length || cache.busy || (!force && Date.now() - cache.at < 4000)) return;
    cache.busy = true; cache.error = null; paint();
    try { cache.data = await api(`/api/v1/pipelines/${encodeURIComponent(detail.pipeline.id)}/insights`); }
    catch (error) { cache.error = error.message; }
    finally { cache.at = Date.now(); cache.busy = false; cache.paint?.(); }
  }
  disclosure.addEventListener("toggle", () => { memo.analysisOpen = disclosure.open; if (disclosure.open) { paint(); void load(); } });
  window.requestAnimationFrame(() => { if (disclosure.open) { paint(); void load(); } });
  return disclosure;
}
