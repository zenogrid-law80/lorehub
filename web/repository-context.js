"use strict";

const REPOSITORY_SECTIONS = ["pipelines", "graphs", "ci-settings", "repository-links", "repository-tree"];
const repositoryBranchSelections = new Map();
const repositoryBranchOptions = new Map();
let repositoryNavigationRequest = 0;
let repositorySelectionRestored = false;
let repositoryListStatus = "loading";
const repositoryContextCopy = {
  choose: ["저장소 선택", "Select repository", "选择仓库"],
  search: ["저장소 검색…", "Search repositories…", "搜索仓库…"],
  empty: ["선택할 저장소가 없습니다", "No repositories available", "没有可选的仓库"],
  noRepositories: ["일치하는 저장소가 없습니다", "No matching repositories", "没有匹配的仓库"],
  loading: ["저장소 불러오는 중…", "Loading repositories…", "正在加载仓库…"],
  retry: ["저장소 다시 불러오기", "Retry loading repositories", "重新加载仓库"],
  scope: ["선택한 저장소", "Selected repository", "所选仓库"],
  back: ["저장소 목록", "Repository list", "仓库列表"],
  branch: ["브랜치", "Branch", "分支"],
  history: ["실행 이력", "Run history", "运行历史"],
  all: ["전체 저장소 보기", "Show all repositories", "查看所有仓库"],
  scopedSearchHint: ["선택한 저장소와 브랜치의 전체 실행 이력에서 검색합니다. 파이프라인은 정확한 이름을 입력하세요.", "Search all run history for the selected repository and branch. Enter an exact pipeline name.", "搜索所选仓库与分支的全部运行历史。请输入准确的流水线名称。"],
  searchHint: ["전체 실행 이력에서 검색합니다. Branch와 파이프라인은 정확한 이름을 입력하세요. 입력 후보는 불러온 이력 기준입니다.", "Search all run history. Enter exact branch and pipeline names. Suggestions come from loaded runs.", "搜索全部运行历史。请输入准确的分支和流水线名称，候选项来自已加载的运行。"],
  noMatches: ["검색 결과가 없습니다", "No matching runs", "没有匹配的运行"],
  changeFilters: ["검색어나 필터를 변경해 다시 검색하세요.", "Change the search text or filters and try again.", "请更改搜索内容或筛选条件后重试。"],
};
function rct(key) { return repositoryContextCopy[key][state.locale === "ko" ? 0 : state.locale === "zh-CN" ? 2 : 1]; }

// The remembered sidebar selection is independent of the current page's scope.
function selectedRepository() { return state.selectedRepository || ""; }
function repositorySelectionKey() { return `lorehub.repository-selection:${state.user?.id || "anonymous"}`; }
function persistRepositorySelection() {
  try {
    const repository = selectedRepository();
    if (repository) localStorage.setItem(repositorySelectionKey(), JSON.stringify({ repository, branch: repositoryBranchFor(repository) }));
    else localStorage.removeItem(repositorySelectionKey());
  } catch (_) { /* Navigation remains usable when browser storage is unavailable. */ }
}
function rememberRepositorySelection(repository) {
  if (!state.repositories.some(item => item.url === repository)) return;
  state.selectedRepository = repository;
  persistRepositorySelection();
  rememberRepositoryVisit(repository);
}
function recentRepositoryVisits() {
  try {
    const saved = JSON.parse(localStorage.getItem(`lorehub.repository-visits:${state.user?.id || "anonymous"}`));
    if (!Array.isArray(saved)) return [];
    const seen = new Set();
    return saved.filter(item => {
      if (!item || typeof item.repository !== "string" || typeof item.branch !== "string" || seen.has(item.repository)) return false;
      if (!state.repositories.some(repository => repository.url === item.repository)) return false;
      seen.add(item.repository); return true;
    }).slice(0, 20);
  } catch (_) { return []; }
}
function rememberRepositoryVisit(repository) {
  if (!state.repositories.some(item => item.url === repository)) return;
  try {
    const visits = recentRepositoryVisits().filter(item => item.repository !== repository);
    visits.unshift({ repository, branch: repositoryBranchFor(repository) });
    localStorage.setItem(`lorehub.repository-visits:${state.user?.id || "anonymous"}`, JSON.stringify(visits.slice(0, 20)));
  } catch (_) { /* Recent visits are optional when browser storage is unavailable. */ }
}
function reconcileRepositorySelection() {
  const available = repository => state.repositories.some(item => item.url === repository);
  if (!repositorySelectionRestored) {
    repositorySelectionRestored = true;
    let saved;
    try { saved = JSON.parse(localStorage.getItem(repositorySelectionKey())); } catch (_) { /* Ignore unavailable or invalid storage. */ }
    if (saved && typeof saved.repository === "string" && available(saved.repository)) {
      state.selectedRepository = saved.repository;
      if (typeof saved.branch === "string") repositoryBranchSelections.set(saved.repository, saved.branch);
    }
    const route = repositoryRoute(window.location.hash);
    if (available(route.repository)) {
      state.selectedRepository = route.repository;
      if (route.branch) repositoryBranchSelections.set(route.repository, route.branch);
    }
  }
  if (!available(selectedRepository())) state.selectedRepository = "";
  persistRepositorySelection();
  try {
    const key = `lorehub.repository-visits:${state.user?.id || "anonymous"}`;
    const visits = recentRepositoryVisits();
    if (visits.length) localStorage.setItem(key, JSON.stringify(visits));
    else localStorage.removeItem(key);
  } catch (_) { /* Ignore unavailable storage. */ }
}
function switchRepository(repository) {
  if (!state.repositories.some(item => item.url === repository)) return;
  const section = state.repositoryScope && REPOSITORY_SECTIONS.includes(state.section) ? state.section : "repository-tree";
  // Commit the selection only in showSection, after unsaved-edit checks succeed.
  navigateRepositorySection(section, repository);
}
function matchingRepositories(query) {
  const search = query.trim().toLocaleLowerCase();
  return state.repositories.filter(item => `${item.name} ${item.url}`.toLocaleLowerCase().includes(search));
}
function createRepositoryPicker(className) {
  const picker = document.createElement("details"); picker.className = `repository-picker ${className}`;
  const summary = document.createElement("summary");
  const search = document.createElement("input"); search.type = "search";
  const results = document.createElement("div"); results.className = "repository-picker-results";
  const status = document.createElement("p"); status.className = "repository-picker-status"; status.setAttribute("role", "status");
  const retry = document.createElement("button"); retry.type = "button"; retry.className = "button button--ghost";
  retry.addEventListener("click", () => void loadRepositories(false));
  picker.append(summary, search, results, status, retry);
  search.addEventListener("input", () => renderRepositoryPicker(picker));
  picker.addEventListener("toggle", () => { if (picker.open) search.focus(); });
  picker.addEventListener("keydown", event => {
    if (event.key === "Escape") { picker.open = false; summary.focus(); }
  });
  return picker;
}
function renderRepositoryPicker(picker) {
  const selected = state.repositories.find(item => item.url === selectedRepository());
  const summary = picker.querySelector("summary");
  summary.textContent = selected?.name || rct("choose");
  summary.setAttribute("aria-label", `${rct("choose")}${selected ? `: ${selected.name}` : ""}`);
  summary.title = selected?.url || rct("choose");
  const input = picker.querySelector("input");
  input.placeholder = rct("search"); input.setAttribute("aria-label", rct("search"));
  const results = picker.querySelector(".repository-picker-results"); results.replaceChildren();
  const matches = matchingRepositories(input.value);
  for (const repository of matches) {
    const button = document.createElement("button"); button.type = "button";
    button.className = "repository-picker-option";
    button.setAttribute("aria-pressed", String(repository.url === selectedRepository()));
    button.title = repository.url;
    button.append(textNode(repository.name, "repository-picker-name"));
    button.addEventListener("click", () => {
      picker.open = false; input.value = ""; summary.focus(); switchRepository(repository.url);
    });
    results.append(button);
  }
  const status = picker.querySelector(".repository-picker-status");
  status.textContent = repositoryListStatus === "loading" ? rct("loading") : matches.length ? "" : rct(state.repositories.length ? "noRepositories" : "empty");
  status.hidden = !status.textContent;
  const retry = picker.querySelector(":scope > button");
  retry.hidden = repositoryListStatus !== "error"; retry.textContent = rct("retry");
}

function repositoryRoute(hash) {
  const [section, query = ""] = hash.replace(/^#/, "").split("?", 2);
  const params = new URLSearchParams(query);
  const repository = REPOSITORY_SECTIONS.includes(section) ? params.get("repository") || "" : "";
  return { section, repository, branch: repository ? params.get("branch") || "" : "" };
}
function repositoryBranchFor(repository) {
  return repository === state.repositoryScope ? state.repositoryBranch || "" : repositoryBranchSelections.get(repository) || "";
}
function repositorySectionHash(section, repository = "", branch = repositoryBranchFor(repository)) {
  const params = new URLSearchParams();
  if (REPOSITORY_SECTIONS.includes(section) && repository) {
    params.set("repository", repository);
    if (branch) params.set("branch", branch);
  }
  return `#${section}${params.size ? `?${params}` : ""}`;
}
function repositoryNavigationValue(section = state.section, scope = state.repositoryScope) {
  return scope && REPOSITORY_SECTIONS.includes(section) ? `repository:${section}` : section;
}
function navigateRepositorySection(section, repository = state.repositoryScope, branch = repositoryBranchFor(repository)) {
  const hash = repositorySectionHash(section, repository, branch);
  if (window.location.hash === hash) void showSection(section);
  else window.location.hash = hash;
}
function rememberRepositoryBranch(branch, repository = state.repositoryScope) {
  if (!repository) return;
  repositoryBranchSelections.set(repository, branch);
  if (repository !== state.repositoryScope) {
    if (repository === selectedRepository()) persistRepositorySelection();
    return;
  }
  state.repositoryBranch = branch;
  rememberRepositoryVisit(repository);
  if (repository === selectedRepository()) persistRepositorySelection();
  const route = repositoryRoute(window.location.hash);
  if (route.repository === repository && route.section === state.section) {
    const params = new URLSearchParams(window.location.hash.split("?").slice(1).join("?"));
    if (branch) params.set("branch", branch); else params.delete("branch");
    const hash = `#${state.section}?${params}`;
    history.replaceState(history.state, "", hash);
    rememberPipelinePage(hash);
  }
  renderRepositoryContext();
}
function selectRepositoryBranch(branches, repository = state.repositoryScope) {
  if (repository) repositoryBranchOptions.set(repository, branches);
  const preferred = repositoryBranchFor(repository);
  const branch = branches.find(item => item.name === preferred) || branches.find(item => item.name === "main") || branches[0];
  rememberRepositoryBranch(branch?.name || "", repository);
  return branch;
}
async function loadRepositoryNavigationBranches() {
  const scope = state.repositoryScope, section = state.section, branch = state.repositoryBranch;
  const request = ++repositoryNavigationRequest;
  if (!scope) return true;
  try {
    const branches = await api(`/api/v1/repositories/${encodeURIComponent(repositoryName(scope))}/branches`);
    if (request !== repositoryNavigationRequest || state.repositoryScope !== scope || state.section !== section || state.repositoryBranch !== branch) return false;
    // Historical runs may refer to branches that no longer exist.
    if (["pipelines", "graphs"].includes(section) && branch && !branches.some(item => item.name === branch)) {
      repositoryBranchOptions.set(scope, branches);
      renderRepositoryContext();
    } else selectRepositoryBranch(branches, scope);
  } catch (error) {
    if (request !== repositoryNavigationRequest || state.repositoryScope !== scope || state.section !== section) return false;
    toast(error.message, "error");
  }
  return true;
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
function createRepositoryBranchSelector(className) {
  const label = document.createElement("label"); label.className = `repository-nav-branch ${className}`; label.hidden = true;
  const caption = document.createElement("span");
  const select = document.createElement("select");
  select.addEventListener("change", () => navigateRepositorySection(state.section, state.repositoryScope, select.value));
  label.append(caption, select);
  return label;
}
function renderRepositoryBranchSelector(label) {
  const scope = state.repositoryScope;
  label.hidden = !scope || !REPOSITORY_SECTIONS.includes(state.section);
  if (label.hidden) return;
  label.querySelector("span").textContent = rct("branch");
  const select = label.querySelector("select"); select.setAttribute("aria-label", rct("branch"));
  select.replaceChildren();
  for (const branch of repositoryBranchOptions.get(scope) || []) select.add(new Option(branch.name, branch.name));
  if (state.repositoryBranch && !Array.from(select.options).some(option => option.value === state.repositoryBranch)) select.add(new Option(state.repositoryBranch, state.repositoryBranch));
  select.value = state.repositoryBranch || ""; select.disabled = !select.options.length;
}
function initRepositoryContext() {
  document.getElementById("repository-picker-slot").append(createRepositoryPicker("repository-picker--sidebar"), createRepositoryBranchSelector("repository-nav-branch--sidebar"));
  const mobile = document.getElementById("mobile-page-select");
  mobile.insertAdjacentElement("beforebegin", createRepositoryPicker("repository-picker--mobile"));
  mobile.insertAdjacentElement("beforebegin", createRepositoryBranchSelector("repository-nav-branch--mobile"));
}
function renderRepositoryContext() {
  const scope = state.repositoryScope;
  const selected = selectedRepository();
  const scoped = Boolean(scope) && REPOSITORY_SECTIONS.includes(state.section);
  document.querySelector("#pipelines-heading h1").textContent = t(scoped ? "Run history" : "All run history");
  const mobile = document.getElementById("mobile-page-select");
  for (const link of document.querySelectorAll(".nav-item[data-section]")) {
    const local = link.dataset.navScope === "repository";
    link.href = repositorySectionHash(link.dataset.section, local ? selected : "");
    link.hidden = local && !selected;
    const active = link.dataset.section === state.section && local === scoped;
    link.classList.toggle("is-active", active);
    if (active) link.setAttribute("aria-current", "page"); else link.removeAttribute("aria-current");
  }
  document.querySelector('[data-nav-group="repository"]').hidden = false;
  for (const picker of document.querySelectorAll(".repository-picker")) renderRepositoryPicker(picker);
  const group = mobile.querySelector('optgroup[data-nav-group="repository"]');
  group.hidden = !selected; group.disabled = !selected;
  group.label = selected ? `${rct("scope")}: ${repositoryName(selected)}` : rct("scope");
  mobile.value = repositoryNavigationValue();
  for (const label of document.querySelectorAll(".repository-nav-branch")) renderRepositoryBranchSelector(label);
}

function pipelineHistoryParameters() {
  const params = new URLSearchParams();
  if (state.repositoryScope) params.set("repository_url", state.repositoryScope);
  if (["overview", "pipelines"].includes(state.section)) {
    if (state.query) params.set("q", state.query);
    if (state.filter && state.filter !== "all") params.set("status", state.filter === "running" ? "active" : state.filter === "executing" ? "running" : state.filter);
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
  state.pipelineBranchFilter = historyPage ? params.get("branch") || state.repositoryBranch || "" : "";
  state.pipelineNameFilter = historyPage ? params.get("pipeline_name") || "" : "";
  const status = historyPage ? params.get("status") : null;
  state.filter = status === "running" ? "executing" : ["active", "queued", "finished", "failed"].includes(status) ? (status === "active" ? "running" : status) : "all";
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
  if (state.repositoryScope && state.pipelineBranchFilter !== state.repositoryBranch) rememberRepositoryBranch(state.pipelineBranchFilter || "");
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
    if (request !== pipelineLocationRequest || pipelineLocation(window.location.hash).id !== location.id) return;
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
