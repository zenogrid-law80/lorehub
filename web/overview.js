const workspaceOverview = { data: null, loading: false, error: "", lastAttempt: 0, request: 0 };
const overviewCopy = {
  description: ["저장소와 CI 현황을 확인하고 최근 작업을 이어가세요.", "Check your repositories and CI, and pick up where you left off.", "查看仓库与 CI 状态，继续最近的工作。"],
  repositories: ["접근 가능한 저장소", "Accessible repositories", "可访问的仓库"],
  running: ["실행 중", "Running", "运行中"],
  queued: ["대기 중", "Queued", "排队中"],
  online: ["온라인 Runner", "Online runners", "在线 Runner"],
  runnersNote: ["전체 워크스페이스", "Across the workspace", "整个工作区"],
  live: ["현재 상태", "Current activity", "当前活动"],
  attention: ["확인이 필요한 항목", "Needs attention", "需要关注"],
  attentionNote: ["각 항목에서 최대 5건을 표시합니다.", "Showing up to 5 items per category.", "每类最多显示 5 项。"],
  failed: ["실패한 실행 · 최근 24시간", "Failed runs · last 24 hours", "失败运行 · 最近 24 小时"],
  waiting: ["5분 이상 대기 중", "Queued for over 5 minutes", "排队超过 5 分钟"],
  links: ["링크 동기화 오류", "Link sync errors", "链接同步错误"],
  recent: ["최근 방문 저장소", "Recently visited repositories", "最近访问的仓库"],
  recentNote: ["이 브라우저에서 마지막으로 사용한 브랜치로 이동합니다.", "Open the last branch you used in this browser.", "打开此浏览器中上次使用的分支。"],
  noVisits: ["최근 방문한 저장소가 없습니다. 저장소를 열어 작업을 시작하세요.", "No recent visits yet. Open a repository to get started.", "暂无访问记录。打开仓库开始工作。"],
  noRepositories: ["접근 가능한 저장소가 없습니다.", "No accessible repositories yet.", "暂无可访问的仓库。"],
  browse: ["저장소 보기", "Browse repositories", "浏览仓库"],
  runs: ["최근 실행", "Recent runs", "最近运行"],
  runsNote: ["접근 가능한 저장소의 최근 실행 5건", "Latest 5 runs across your accessible repositories", "可访问仓库中最近的 5 次运行"],
  error: ["워크스페이스 현황을 불러오지 못했습니다.", "Could not load workspace activity.", "无法加载工作区活动。"],
  retry: ["다시 시도", "Retry", "重试"],
  repositoryError: ["저장소 목록을 불러오지 못했습니다.", "Could not load repositories.", "无法加载仓库。"],
};
function ovt(key) { return overviewCopy[key][state.locale === "ko" ? 0 : state.locale === "zh-CN" ? 2 : 1]; }

async function loadOverview() {
  if (workspaceOverview.loading) return;
  const request = ++workspaceOverview.request;
  workspaceOverview.loading = true;
  workspaceOverview.lastAttempt = Date.now();
  renderOverview();
  try {
    const data = await api("/api/v1/overview");
    if (request !== workspaceOverview.request) return;
    workspaceOverview.data = data;
    workspaceOverview.error = "";
  } catch (_) {
    if (request !== workspaceOverview.request) return;
    workspaceOverview.data = null;
    workspaceOverview.error = ovt("error");
  } finally {
    if (request === workspaceOverview.request) {
      workspaceOverview.loading = false;
      renderOverview();
    }
  }
}

function overviewLink(label, section, repository = "", branch = "", filters = {}) {
  const link = document.createElement("a");
  link.textContent = label;
  const hash = repositorySectionHash(section, repository, branch);
  const [path, search = ""] = hash.split("?");
  const params = new URLSearchParams(search);
  for (const [key, value] of Object.entries(filters)) params.set(key, value);
  link.href = `${path}${params.size ? `?${params}` : ""}`;
  return link;
}

function overviewHeader(title, description, action) {
  const header = document.createElement("div"); header.className = "panel-header";
  const copy = document.createElement("div");
  const h2 = document.createElement("h2"); h2.textContent = title;
  const p = document.createElement("p"); p.textContent = description;
  copy.append(h2, p); header.append(copy);
  if (action) { action.className = "button button--ghost"; header.append(action); }
  return header;
}

function renderOverview() {
  if (state.section !== "overview") return;
  const { data, loading, error } = workspaceOverview;
  document.getElementById("overview-description").textContent = ovt("description");
  const notice = document.getElementById("overview-notice");
  notice.replaceChildren(); notice.hidden = !error;
  if (error) {
    notice.append(textNode(ovt("error")));
    const retry = document.createElement("button"); retry.type = "button"; retry.className = "button button--ghost";
    retry.textContent = ovt("retry"); retry.disabled = loading;
    retry.addEventListener("click", () => void loadOverview()); notice.append(retry);
  }
  const summary = document.getElementById("overview-summary");
  summary.setAttribute("aria-label", t("Overview"));
  summary.setAttribute("aria-busy", String(loading)); summary.replaceChildren();
  for (const [key, label, section, filters, tone] of [
    ["repositories", "repositories", "repositories", {}, "total"],
    ["running", "running", "pipelines", { status: "running" }, "success"],
    ["queued", "queued", "pipelines", { status: "queued" }, "active"],
    ["online_runners", "online", "runners", {}, "total"],
  ]) {
    const card = overviewLink("", section, "", "", filters);
    card.className = `stat-card stat-card--${tone}`;
    const copy = document.createElement("div");
    const labelNode = document.createElement("p"); labelNode.textContent = ovt(label);
    const value = document.createElement("strong"); value.textContent = data ? String(data.summary[key]) : "—";
    const note = document.createElement("small"); note.textContent = ovt(key === "online_runners" ? "runnersNote" : key === "repositories" ? "browse" : "live");
    copy.append(labelNode, value, note); card.append(copy); summary.append(card);
  }
  const attention = document.getElementById("overview-attention"); attention.replaceChildren();
  attention.hidden = !data || !(data.summary.failed || data.summary.waiting || data.summary.link_errors);
  if (!attention.hidden) {
    attention.append(overviewHeader(ovt("attention"), ovt("attentionNote")));
    const groups = document.createElement("div"); groups.className = "overview-alert-groups";
    for (const [key, countKey] of [["failed", "failed"], ["waiting", "waiting"], ["links", "link_errors"]]) {
      if (!data.summary[countKey]) continue;
      const group = document.createElement("div"); group.className = `overview-alert-group overview-alert-group--${key}`;
      const title = document.createElement("h3"); title.textContent = `${ovt(key)} · ${data.summary[countKey]}`; group.append(title);
      for (const item of data[key]) {
        let action;
        if (key === "links") {
          action = overviewLink("", "repository-links", item.repository_url, item.branch);
        } else {
          action = document.createElement("button"); action.type = "button";
          action.addEventListener("click", () => void openPipeline(item.id));
        }
        action.className = "overview-alert-row";
        action.append(textNode(`${repositoryName(item.repository_url)} / ${key === "links" ? item.path : item.pipeline_name || item.id.slice(0, 8)}`));
        if (item.branch) action.append(textNode(item.branch, "overview-row-meta"));
        group.append(action);
      }
      groups.append(group);
    }
    attention.append(groups);
  }
  renderOverviewRepositories();
}

function renderOverviewRepositories() {
  if (state.section !== "overview") return;
  const panel = document.getElementById("overview-repositories"); panel.replaceChildren();
  panel.append(overviewHeader(ovt("recent"), ovt("recentNote"), overviewLink(ovt("browse"), "repositories")));
  const list = document.createElement("div"); list.className = "overview-repository-list";
  const visits = recentRepositoryVisits().slice(0, 6);
  if (repositoryListStatus !== "ready" || !visits.length) {
    const key = repositoryListStatus === "error" ? "repositoryError" : state.repositories.length ? "noVisits" : "noRepositories";
    list.append(textNode(repositoryListStatus === "loading" ? t("dynamic.loading") : ovt(key), "overview-empty"));
  } else {
    for (const visit of visits) {
      const link = overviewLink("", "repository-tree", visit.repository, visit.branch);
      link.className = "overview-repository-card"; link.title = visit.repository;
      link.append(textNode(repositoryName(visit.repository), "overview-repository-name"));
      link.append(textNode(visit.branch || t("Branch"), "overview-row-meta"));
      list.append(link);
    }
  }
  panel.append(list);
}
