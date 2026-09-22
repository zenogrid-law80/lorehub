"use strict";

const REPOSITORY_SECTIONS = ["pipelines", "graphs", "ci-settings", "repository-links"];
const repositoryContextCopy = {
  scope: ["선택한 저장소", "Selected repository", "所选仓库"],
  history: ["실행 이력", "Run history", "运行历史"],
  all: ["전체 저장소 보기", "Show all repositories", "查看所有仓库"],
  searchHint: ["전체 실행 이력에서 검색합니다. Branch와 파이프라인은 정확한 이름을 입력하세요. 입력 후보는 불러온 이력 기준입니다.", "Search all run history. Enter exact branch and pipeline names. Suggestions come from loaded runs.", "搜索全部运行历史。请输入准确的分支和流水线名称，候选项来自已加载的运行。"],
  noMatches: ["검색 결과가 없습니다", "No matching runs", "没有匹配的运行"],
  changeFilters: ["검색어나 필터를 변경해 다시 검색하세요.", "Change the search text or filters and try again.", "请更改搜索内容或筛选条件后重试。"],
};
function rct(key) { return repositoryContextCopy[key][state.locale === "ko" ? 0 : state.locale === "zh-CN" ? 2 : 1]; }

function repositoryRoute(hash) {
  const [section, query = ""] = hash.replace(/^#/, "").split("?", 2);
  return { section, repository: REPOSITORY_SECTIONS.includes(section) ? new URLSearchParams(query).get("repository") || "" : "" };
}
function repositorySectionHash(section, repository = "") {
  const query = REPOSITORY_SECTIONS.includes(section) && repository ? `?${new URLSearchParams({ repository })}` : "";
  return `#${section}${query}`;
}
function navigateRepositorySection(section, repository = state.repositoryScope) {
  const hash = repositorySectionHash(section, repository);
  if (window.location.hash === hash) void showSection(section);
  else window.location.hash = hash;
}
function chooseRepositorySection(section, name) {
  const repository = state.repositories.find(item => item.name === name);
  if (repository) navigateRepositorySection(section, repository.url);
}
function scopedRepositories() {
  return state.repositories.filter(repository => !state.repositoryScope || repository.url === state.repositoryScope);
}
function repositoryQuery(scope = state.repositoryScope) {
  return scope ? `&repository_url=${encodeURIComponent(scope)}` : "";
}
function initRepositoryContext() {
  const section = document.createElement("section");
  section.id = "repository-context";
  section.className = "repository-context";
  section.hidden = true;
  document.getElementById("mobile-page-select").insertAdjacentElement("afterend", section);
}
function renderRepositoryContext() {
  const scope = state.repositoryScope;
  for (const link of document.querySelectorAll(".nav-item[data-section]")) {
    link.href = repositorySectionHash(link.dataset.section, scope);
  }
  const section = document.getElementById("repository-context");
  section.replaceChildren();
  section.hidden = !scope || !REPOSITORY_SECTIONS.includes(state.section);
  if (section.hidden) return;
  section.setAttribute("aria-label", rct("scope"));
  const identity = document.createElement("div");
  identity.append(textNode(rct("scope"), "repository-context-label"), textNode(repositoryName(scope), "repository-context-name"), textNode(scope, "repository-context-url"));
  const nav = document.createElement("nav"); nav.setAttribute("aria-label", rct("scope"));
  for (const [target, label] of [["pipelines", rct("history")], ["graphs", t("Execution graphs")], ["ci-settings", t("CI configuration")], ["repository-links", t("Repository links")]]) {
    const link = document.createElement("a"); link.className = "button button--ghost";
    link.href = repositorySectionHash(target, scope); link.textContent = label;
    if (target === state.section) link.setAttribute("aria-current", "page");
    nav.append(link);
  }
  const clear = document.createElement("a"); clear.className = "button button--ghost";
  clear.href = repositorySectionHash(state.section); clear.textContent = rct("all"); nav.append(clear);
  section.append(identity, nav);
}

function pipelineHistoryParameters() {
  const params = new URLSearchParams();
  if (state.repositoryScope) params.set("repository_url", state.repositoryScope);
  if (["overview", "pipelines"].includes(state.section)) {
    if (state.query) params.set("q", state.query);
    if (state.filter && state.filter !== "all") params.set("status", state.filter === "running" ? "active" : state.filter);
    if (state.section === "pipelines") {
      if (state.pipelineBranchFilter) params.set("branch", state.pipelineBranchFilter);
      if (state.pipelineNameFilter) params.set("pipeline_name", state.pipelineNameFilter);
    }
  }
  return params;
}

function restorePipelineHistoryFilters(hash) {
  const params = new URLSearchParams(hash.split("?").slice(1).join("?"));
  const historyPage = state.section === "pipelines";
  state.query = historyPage ? params.get("q") || "" : "";
  state.pipelineBranchFilter = historyPage ? params.get("branch") || "" : "";
  state.pipelineNameFilter = historyPage ? params.get("pipeline_name") || "" : "";
  const status = historyPage ? params.get("status") : null;
  state.filter = ["active", "finished", "failed"].includes(status) ? (status === "active" ? "running" : status) : "all";
  elements["pipeline-search"].value = state.query;
  document.querySelectorAll(".status-tab").forEach(tab => tab.classList.toggle("is-active", tab.dataset.status === state.filter));
}

function cancelPipelineHistorySearch() {
  window.clearTimeout(state.pipelineSearchTimer);
  state.pipelineSearchTimer = null;
}

function invalidatePipelineHistory() {
  cancelPipelineHistorySearch();
  state.pipelineRequest++;
  state.pipelineLoading = false;
  state.pipelineHasOlderPages = false;
  state.pipelineNextBefore = null;
  state.pipelines = [];
  elements["load-more-pipelines"].hidden = true;
}

function searchPipelineHistory(delay = 0) {
  if (state.section === "pipelines") {
    const params = pipelineHistoryParameters();
    params.delete("repository_url");
    if (state.repositoryScope) params.set("repository", state.repositoryScope);
    const hash = `#pipelines${params.size ? `?${params}` : ""}`;
    history.replaceState(null, "", hash);
    rememberPipelinePage(hash);
  }
  invalidatePipelineHistory();
  state.pipelineSearchTimer = window.setTimeout(() => {
    state.pipelineSearchTimer = null;
    void loadPipelines(false);
  }, delay);
  renderPipelines();
}

// A detail is a single browser-history entry over the current page. Related
// runs replace that entry so Back returns to the original list, including pages
// already loaded in memory. A reloaded/shared entry closes locally, not off-site.
const pipelineNavigationOwner = `${Date.now()}-${Math.random()}`;
let pipelinePageHash = null;
let pipelineLocationRequest = 0;

function pipelineLocation(hash) {
  const [section, ...query] = hash.replace(/^#/, "").split("?");
  const params = new URLSearchParams(query.join("?"));
  const value = params.get("run");
  const id = value && /^[a-zA-Z0-9-]{1,128}$/.test(value) ? value : null;
  params.delete("run");
  return { id, base: `#${section || "overview"}${params.size ? `?${params}` : ""}` };
}

function rememberPipelinePage(hash) { pipelinePageHash = pipelineLocation(hash).base; }

function pipelinePermalink(id) {
  const url = new URL(window.location.href);
  url.username = "";
  url.password = "";
  url.search = "";
  url.hash = `pipelines?${new URLSearchParams({ run: id })}`;
  return url.href;
}

function recordPipelineLocation(id) {
  const current = pipelineLocation(window.location.hash);
  if (current.id === id) return;
  const hash = `${current.base}${current.base.includes("?") ? "&" : "?"}${new URLSearchParams({ run: id })}`;
  if (current.id) history.replaceState(history.state, "", hash);
  else history.pushState({ ...history.state, lorehubDetail: { owner: pipelineNavigationOwner, base: current.base } }, "", hash);
}

function closePipelineLocation(id) {
  const current = pipelineLocation(window.location.hash);
  if (current.id !== id) return;
  const origin = history.state?.lorehubDetail;
  if (origin?.owner === pipelineNavigationOwner && origin.base === current.base) history.back();
  else {
    const data = { ...history.state }; delete data.lorehubDetail;
    history.replaceState(data, "", current.base);
  }
}

function dismissPipelineDetail() {
  executionDetailRequest++;
  state.selectedId = null;
  state.detailLogView = null;
  if (elements["pipeline-detail-dialog"].open) elements["pipeline-detail-dialog"].close();
}

async function syncPipelineLocation(forcePage = false) {
  if (!state.user) return;
  const request = ++pipelineLocationRequest;
  const hash = window.location.hash;
  const location = pipelineLocation(hash);
  if (forcePage || location.base !== pipelinePageHash) {
    dismissPipelineDetail();
    await showSection(sectionFromHash());
    if (request !== pipelineLocationRequest || window.location.hash !== hash) return;
  }
  if (location.id) {
    if (state.selectedId !== location.id || !elements["pipeline-detail-dialog"].open) await openPipeline(location.id, { fromLocation: true });
  } else dismissPipelineDetail();
}

function rememberPipelineLogin() {
  try {
    const hash = window.location.hash;
    if (pipelineLocation(hash).id && hash.length <= 8192) window.sessionStorage.setItem("lorehub_pending_run", JSON.stringify({ hash, at: Date.now() }));
    else window.sessionStorage.removeItem("lorehub_pending_run");
  } catch (_) { /* Direct links remain usable when tab storage is unavailable. */ }
}

function restorePipelineLogin() {
  try {
    const raw = window.sessionStorage.getItem("lorehub_pending_run");
    window.sessionStorage.removeItem("lorehub_pending_run");
    if (!raw || (window.location.hash && window.location.hash !== "#")) return;
    const pending = JSON.parse(raw);
    const age = Date.now() - pending.at;
    if (typeof pending.hash === "string" && pending.hash.startsWith("#") && pending.hash.length <= 8192 && Number.isFinite(age) && age >= 0 && age <= 600000 && pipelineLocation(pending.hash).id) {
      history.replaceState(null, "", pending.hash);
    }
  } catch (_) { /* A blocked or expired login return must not prevent sign-in. */ }
}
