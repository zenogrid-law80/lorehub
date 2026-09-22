"use strict";

const operations = { snapshot: null, error: null, loading: false, request: 0, lastAttempt: 0 };
const operationsCopy = {
  title: ["운영 상태", "Operations", "运行状态"],
  refresh: ["새로고침", "Refresh", "刷新"],
  intro: ["30초마다 대기 작업, Runner와 저장소 확인 결과를 갱신합니다.", "Queue, Runner and repository checks refresh every 30 seconds.", "每 30 秒刷新队列、Runner 和仓库检查结果。"],
  observed: ["관측 시각", "Observed at", "观测时间"],
  queued: ["대기 작업", "Queued", "排队任务"], running: ["실행 중", "Running", "运行中"],
  oldest: ["가장 긴 대기", "Oldest wait", "最长等待"], expired: ["만료된 실행 lease", "Expired execution leases", "已过期的执行租约"],
  online: ["연결된 Runner", "Online Runners", "在线 Runner"], idle: ["유휴 Runner", "Idle Runners", "空闲 Runner"],
  offline: ["연결 끊김", "Disconnected", "连接断开"], stopped: ["종료됨", "Stopped", "已停止"],
  queue: ["대기 원인", "Queue reasons", "等待原因"], repo: ["저장소", "Repository", "仓库"],
  pipeline: ["파이프라인", "Pipeline", "流水线"], os: ["대상 OS", "Target OS", "目标 OS"], wait: ["대기 시간", "Wait", "等待时间"], reason: ["현재 관측", "Current observation", "当前观测"],
  canceling: ["취소 처리 중", "Canceling", "正在取消"], dependencies: ["선행 실행 결과 대기", "Waiting on upstream results", "等待上游结果"],
  no_runner: ["대상 OS에 연결된 Runner 없음", "No online Runner for this OS", "目标 OS 无在线 Runner"],
  draining: ["대상 Runner의 배정이 모두 중지됨", "All matching Runners paused", "匹配的 Runner 均已暂停分配"],
  maintenance: ["배정 중지", "Assignments paused", "已暂停分配"],
  runnerHint: ["배정 중지 수는 오프라인 Runner도 포함합니다. 유휴 수에서는 배정 중지된 Runner를 제외합니다.", "Paused counts include offline Runners. Idle counts exclude paused Runners.", "暂停数量包含离线 Runner。空闲数量不包含暂停分配的 Runner。"],
  busy: ["대상 Runner가 모두 실행 중", "Matching Runners are busy", "匹配的 Runner 均忙碌"],
  ready: ["수행 요청 가능한 상태", "Ready to claim", "可领取任务"],
  queueHint: ["가장 오래 대기한 100건까지 표시합니다. 원인은 관측 시점 기준이며, 수행 가능한 상태에서도 다음 Runner 요청까지 대기할 수 있습니다.", "Shows up to 100 oldest queued runs. Reasons are observations; ready runs can still await the next Runner request.", "最多显示最早的 100 个排队任务。原因基于观测时刻；可领取的任务仍可能等待下一次 Runner 请求。"],
  runners: ["OS별 Runner 가용성", "Runner capacity by OS", "各 OS 的 Runner 容量"],
  watches: ["저장소 확인 상태", "Repository checks", "仓库检查"], state: ["상태", "State", "状态"],
  checked: ["최근 확인", "Last check", "最近检查"], success: ["최근 성공", "Last success", "最近成功"],
  failures: ["연속 실패", "Consecutive failures", "连续失败"], linkErrors: ["링크 갱신 오류", "Link update errors", "链接更新错误"],
  ok: ["확인 정상", "Check succeeded", "检查成功"], error: ["확인 실패", "Check failed", "检查失败"],
  stale: ["확인 지연 (2분 초과)", "Check overdue (>2 min)", "检查延迟（超过 2 分钟）"], unknown: ["아직 관측되지 않음", "Not observed yet", "尚未观测"],
  watch_failed: ["연결 또는 CI 확인 실패", "Connection or CI check failed", "连接或 CI 检查失败"],
  link_index_failed: ["링크 목록 확인 실패", "Link indexing failed", "链接索引失败"],
  backend_unavailable: ["저장소 backend 설정 확인 필요", "Storage backend unavailable", "存储后端不可用"],
  watchHint: ["최근 확인 완료 시각을 표시합니다. 2분 초과는 실제 revision 차이를 뜻하지 않습니다. 링크 오류 수는 현재 링크에 남아 있는 최근 오류입니다.", "Shows completed check times. Overdue checks do not measure revision lag. Link errors count the latest stored errors for current links.", "显示检查完成时间。检查延迟不代表修订差距。链接错误数表示当前链接最近保存的错误。"],
  shown: ["표시 / 전체 저장소", "Shown / total repositories", "显示 / 全部仓库"],
  storage: ["PostgreSQL 사용량", "PostgreSQL storage", "PostgreSQL 存储"],
  database: ["전체 DB", "Entire database", "整个数据库"], execution: ["실행·job 기록", "Executions and jobs", "执行和任务记录"], logs: ["로그", "Logs", "日志"],
  storageHint: ["인덱스와 TOAST를 포함한 물리 용량입니다. DB 전체 값에는 아래 항목이 포함됩니다. 디스크 여유 공간과 Lore 저장소 용량은 포함하지 않습니다.", "Physical sizes include indexes and TOAST. Database size includes the other values. Free disk space and Lore storage are not measured.", "物理大小包含索引和 TOAST。数据库大小包含其他值。不测量可用磁盘空间和 Lore 存储。"],
  empty: ["표시할 항목이 없습니다.", "No items to display.", "没有可显示的项目。"],
  unavailable: ["최신 상태를 가져오지 못했습니다. 이전 데이터는 현재 상태로 표시하지 않습니다.", "Could not load current status. Previous data is not displayed as current.", "无法获取当前状态。不会将旧数据作为当前状态显示。"],
};
function ot(key) { return operationsCopy[key]?.[state.locale === "ko" ? 0 : state.locale === "zh-CN" ? 2 : 1] ?? key; }
function operationsAge(seconds) {
  if (seconds === null || !Number.isFinite(seconds)) return "—";
  return `${Math.floor(Math.max(0, seconds) / 60)}m ${Math.floor(Math.max(0, seconds) % 60)}s`;
}
function operationsBytes(bytes) {
  if (!Number.isFinite(bytes)) return "—";
  const unit = bytes >= 1024 ** 3 ? 3 : bytes >= 1024 ** 2 ? 2 : bytes >= 1024 ? 1 : 0;
  return `${(bytes / 1024 ** unit).toFixed(unit ? 1 : 0)} ${["B", "KiB", "MiB", "GiB"][unit]}`;
}
function operationsTime(time) { return time ? new Date(time).toLocaleString(localeTag()) : "—"; }

async function loadOperations() {
  if (operations.loading) return;
  const request = ++operations.request;
  operations.loading = true;
  operations.lastAttempt = Date.now();
  if (!operations.snapshot && state.section === "operations") renderOperations();
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), 15000);
  try {
    const snapshot = await api("/api/v1/operations", { signal: controller.signal });
    if (request !== operations.request) return;
    operations.snapshot = snapshot;
    operations.error = null;
  } catch (error) {
    if (request !== operations.request) return;
    operations.snapshot = null;
    operations.error = error.message;
  } finally {
    window.clearTimeout(timeout);
    operations.loading = false;
    if (state.section === "operations" && request === operations.request) renderOperations();
  }
}

function operationsTable(page, title, headers, rows, hint) {
  const panel = mn("section", "pipeline-panel");
  const heading = mn("header", "panel-header"); heading.append(mn("h2", "", ot(title))); panel.append(heading);
  if (hint) panel.append(mn("p", "management-note", hint));
  if (!rows.length) panel.append(mn("p", "management-note", ot("empty")));
  else {
    const wrap = mn("div", "table-wrap"); const table = mn("table", "management-table");
    const head = mn("thead"); const tr = mn("tr");
    for (const label of headers) { const th = mn("th", "", ot(label)); th.scope = "col"; tr.append(th); }
    head.append(tr); table.append(head); const body = mn("tbody");
    for (const row of rows) {
      const tr = mn("tr");
      for (const value of row) { const td = mn("td"); td.append(typeof value === "object" ? value : document.createTextNode(String(value ?? "—"))); tr.append(td); }
      body.append(tr);
    }
    table.append(body); wrap.append(table); panel.append(wrap);
  }
  page.append(panel);
}

function renderOperations() {
  const page = document.getElementById("operations-page"); page.replaceChildren();
  const heading = mn("header", "page-heading"); const copy = mn("div");
  copy.append(mn("h1", "", ot("title")), mn("p", "", ot("intro")));
  heading.append(copy, mb(operations.error ? mt("retry") : ot("refresh"), () => void loadOperations())); page.append(heading);
  if (!operations.snapshot) {
    page.append(mn("p", "management-note", operations.error ? ot("unavailable") : mt("loading")));
    return;
  }
  const s = operations.snapshot;
  page.append(mn("p", "management-note", `${ot("observed")}: ${operationsTime(s.observed_at)}`));
  const summary = mn("dl", "operations-summary");
  for (const [label, value] of [["queued", s.queue.queued], ["oldest", operationsAge(s.queue.oldest_wait_seconds)], ["running", s.queue.running], ["expired", s.queue.expired_leases]]) {
    const card = mn("div"); card.append(mn("dt", "", ot(label)), mn("dd", "", String(value))); summary.append(card);
  }
  page.append(summary);
  operationsTable(page, "queue", ["repo", "pipeline", "os", "wait", "reason"], s.waiting.map(p => [repositoryName(p.repository_url), mb(p.pipeline_name || p.id.slice(0, 8), () => void openPipeline(p.id)), p.runner_os || "—", operationsAge(p.wait_seconds), ot(p.reason)]), ot("queueHint"));
  operationsTable(page, "runners", ["os", "online", "idle", "maintenance", "offline", "stopped"], s.runners.map(r => [r.os, r.online, r.idle, r.draining, r.offline, r.stopped]), ot("runnerHint"));
  operationsTable(page, "watches", ["repo", "state", "checked", "success", "failures", "linkErrors"], s.repositories.map(r => [r.name, `${ot(r.state)}${r.error_code ? ` · ${ot(r.error_code)}` : ""}`, operationsTime(r.checked_at), operationsTime(r.last_success_at), r.consecutive_failures, r.link_errors]), `${ot("shown")}: ${s.repositories.length} / ${s.repository_count}. ${ot("watchHint")}`);
  operationsTable(page, "storage", ["database", "execution", "logs"], [[operationsBytes(s.storage.database_bytes), operationsBytes(s.storage.execution_bytes), operationsBytes(s.storage.log_bytes)]], ot("storageHint"));
}
