"use strict";

const MANAGEMENT_SECTIONS = ["accounts", "account-groups", "repository-access", "workspace-views", "operations"];
const management = { accounts: [], groups: [], repositories: [], repositoryAccess: { repositories: [], groups: [] }, views: [], loaded: false, request: 0, dirty: false };
const managementCopy = {
  operationsMenu: ["운영 상태", "Operations", "运行状态"],
  accounts: ["계정 관리", "Accounts", "账号管理"], groups: ["계정 그룹 관리", "Account groups", "账号组管理"], repositoryAccess: ["Repository 접근 권한", "Repository access", "仓库访问权限"], views: ["Sparse View", "Sparse View", "稀疏视图"],
  accountMenu: ["계정", "Accounts", "账号"], groupMenu: ["계정 그룹", "Account groups", "账号组"], accessMenu: ["Repository 권한", "Repository access", "仓库权限"], viewMenu: ["Sparse View", "Sparse View", "稀疏视图"],
  accountIntro: ["조직 계정을 확인하고 관리자는 계정 등급을 변경할 수 있습니다.", "Browse organization accounts. Administrators can change account roles.", "查看组织账号。管理员可以更改账号等级。"],
  groupIntro: ["함께 작업할 계정을 그룹으로 묶고 구성원을 관리합니다. 관리자는 모든 그룹을 볼 수 있습니다.", "Organize accounts into groups and manage membership. Administrators can view every group.", "将账号整理为组并管理成员。管理员可以查看所有组。"],
  accessIntro: ["Repository와 Account Group을 연결해 그룹 구성원에게 접근 권한을 부여합니다.", "Connect repositories to account groups to grant access to every group member.", "将仓库连接到账号组，为所有组成员授予访问权限。"],
  viewIntro: ["CI가 필요한 파일만 받을 수 있도록 Sparse View를 관리합니다. 파이프라인은 .lore-ci.toml에서 View 이름을 지정합니다.", "Manage Sparse Views so CI fetches only the files it needs. Pipelines select a View by name in .lore-ci.toml.", "管理稀疏视图，让 CI 只获取所需文件。流水线在 .lore-ci.toml 中按名称指定视图。"],
  profile: ["내 프로필", "My profile", "我的资料"], name: ["표시 이름", "Display name", "显示名称"], email: ["이메일", "Email", "电子邮件"], login: ["최근 로그인", "Last sign-in", "最近登录"], joined: ["가입일", "Joined", "加入时间"],
  role: ["등급", "Role", "等级"], roleUser: ["일반 사용자", "Regular user", "普通用户"], roleAdmin: ["관리자", "Administrator", "管理员"], roleHint: ["관리자만 계정 등급을 변경할 수 있습니다. 그룹 소유자는 소유권을 이전한 뒤 일반 사용자로 변경할 수 있습니다. 마지막 관리자는 변경할 수 없습니다.", "Only administrators can change roles. Transfer group ownership before changing an owner to a regular user; the last administrator cannot be demoted.", "只有管理员可以更改等级。组所有者降为普通用户前必须转让所有权，最后一名管理员不能降级。"],
  save: ["저장", "Save", "保存"], saved: ["저장했습니다.", "Saved.", "已保存。"], saving: ["저장 중…", "Saving…", "正在保存…"], cancel: ["취소", "Cancel", "取消"], edit: ["수정", "Edit", "编辑"], remove: ["삭제", "Delete", "删除"], close: ["닫기", "Close", "关闭"],
  newGroup: ["새 그룹", "New group", "新建组"], groupName: ["그룹 이름", "Group name", "组名称"], description: ["설명", "Description", "描述"], members: ["구성원", "Members", "成员"], owner: ["소유자", "Owner", "所有者"], member: ["구성원", "Member", "成员"], adminView: ["관리자 열람", "Administrator view", "管理员查看"], me: ["나", "You", "您"],
  emptyAccounts: ["검색 결과가 없습니다.", "No matching accounts.", "没有匹配的账号。"], emptyGroups: ["표시할 그룹이 없습니다. 새 그룹을 만들어 시작하세요.", "No groups to display. Create a group to get started.", "暂无组。创建一个组以开始。"],
  groupHint: ["현재 소유자만 그룹을 수정·삭제하거나 소유권을 넘길 수 있습니다. 소유자는 항상 구성원입니다.", "The current owner can edit, delete or transfer the group. The owner is always a member.", "只有当前所有者可以编辑、删除或转让组。所有者始终是成员。"],
  groupSelect: ["계정 그룹", "Account group", "账号组"], repoSelect: ["리포지토리", "Repository", "仓库"], chooseGroup: ["그룹을 선택하세요", "Choose a group", "选择组"], chooseRepo: ["리포지토리를 선택하세요", "Choose a repository", "选择仓库"],
  noGroup: ["먼저 계정 그룹을 생성하거나 그룹에 참여하세요.", "Create or join an account group first.", "请先创建或加入账号组。"],
  noRepo: ["설정 가능한 리포지토리가 없습니다. 리포지토리 소유자와 관리자가 설정을 추가할 수 있습니다.", "No repositories available. Repository owners and administrators can add presets.", "暂无可用仓库。仓库所有者和管理员可以添加预设。"],
  accessHint: ["Repository 소유자와 관리자가 권한을 설정할 수 있습니다. 삭제 권한은 공유되지 않으며, 그룹에서 제거된 계정의 접근 권한은 즉시 회수됩니다.", "Repository owners and administrators manage access. Delete permission is never shared, and removing an account from a group revokes access immediately.", "仓库所有者和管理员可以管理访问权限。删除权限不会共享，账号移出组后访问权限会立即撤销。"],
  grantedGroups: ["접근 가능한 그룹", "Groups with access", "拥有访问权限的组"],
  saveAccess: ["권한 저장", "Save access", "保存权限"],
  noManageableRepositories: ["권한을 설정할 수 있는 Repository가 없습니다.", "No repositories are available for access management.", "没有可管理访问权限的仓库。"],
  full: ["전체 workspace", "Full workspace", "完整工作区"], sparse: ["Sparse workspace", "Sparse workspace", "稀疏工作区"], mode: ["Workspace 범위", "Workspace scope", "工作区范围"],
  rules: ["View 규칙", "View rules", "视图规则"], ruleHint: ["한 줄에 규칙 하나. 일반 패턴은 제외, ! 패턴은 포함입니다. 뒤의 규칙이 우선하며 #은 주석입니다.", "One rule per line. Patterns exclude; ! patterns include. Later rules win; # starts a comment.", "每行一条规则。普通模式排除，! 模式包含。后面的规则优先；# 表示注释。"],
  presetHint: ["Sparse View는 리포지토리별 CI 프리셋입니다. .lore-ci.toml의 파이프라인에 sparse_view = \"View 이름\"을 지정하세요.", "Sparse Views are repository-specific CI presets. Set sparse_view = \"View name\" in a pipeline in .lore-ci.toml.", "稀疏视图是按仓库管理的 CI 预设。在 .lore-ci.toml 的流水线中设置 sparse_view = \"视图名称\"。"],
  fullHint: ["모든 경로를 포함합니다. 다운로드한 view 파일은 비어 있습니다.", "Includes all paths. The downloaded view file is empty.", "包含所有路径。下载的视图文件为空。"],
  download: ["view 파일 다운로드", "Download view file", "下载视图文件"], applyHint: ["다운로드한 파일은 새 clone에서 lore repository clone --view <파일> <URL>로 사용하세요.", "Use the downloaded file with lore repository clone --view <file> <URL> for a new clone.", "新克隆时使用 lore repository clone --view <文件> <URL>。"],
  unsaved: ["저장되지 않음", "Not saved", "未保存"], stored: ["저장된 설정", "Saved preset", "已保存预设"], readonly: ["그룹 구성원은 저장된 설정을 확인하고 다운로드할 수 있습니다.", "Group members can view and download saved presets.", "组成员可以查看和下载已保存的预设。"],
  deleteGroup: ["이 그룹을 삭제하면 그룹을 통한 Repository 접근 권한도 해제됩니다. 삭제할까요?", "Deleting this group also removes its repository access grants. Delete the group?", "删除此组也会移除通过该组授予的仓库访问权限。确认删除？"],
  deleteView: ["이 Sparse View를 삭제할까요? 이미 생성된 CI 실행의 View 규칙은 유지됩니다.", "Delete this Sparse View? View rules already saved in CI runs remain unchanged.", "删除此稀疏视图？已创建的 CI 运行中保存的视图规则保持不变。"],
  discard: ["저장하지 않은 변경사항을 버릴까요?", "Discard unsaved changes?", "放弃未保存的更改？"],
  loading: ["불러오는 중…", "Loading…", "正在加载…"], retry: ["다시 시도", "Retry", "重试"],
  search: ["계정 또는 그룹 검색…", "Search accounts or groups…", "搜索账号或组…"], accessSearch: ["Repository 또는 그룹 검색…", "Search repositories or groups…", "搜索仓库或组…"], viewSearch: ["Sparse View 검색…", "Search Sparse Views…", "搜索稀疏视图…"], navigation: ["페이지 이동", "Go to page", "前往页面"],
  invalidRules: ["10,000바이트 이하의 규칙을 입력하세요. Sparse 모드는 주석 외 규칙이 필요합니다.", "Enter at most 10,000 bytes of rules. Sparse mode requires a non-comment rule.", "规则不能超过 10,000 字节。稀疏模式需要非注释规则。"],
  viewList: ["Sparse View 목록", "Sparse View library", "稀疏视图列表"], newView: ["새 Sparse View", "New Sparse View", "新建稀疏视图"], editView: ["Sparse View 수정", "Edit Sparse View", "编辑稀疏视图"], viewName: ["View 이름", "View name", "视图名称"],
  emptyViews: ["표시할 Sparse View가 없습니다.", "No Sparse Views to display.", "没有可显示的稀疏视图。"]
};
function mt(key) { return managementCopy[key][state.locale === "ko" ? 0 : state.locale === "zh-CN" ? 2 : 1]; }
function mn(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}
function mb(label, action, style = "ghost") {
  const button = mn("button", `button button--${style}`, label); button.type = "button";
  button.addEventListener("click", action); return button;
}
function field(label, control, hint) {
  const node = mn("label", "field"); node.append(mn("span", "", label), control);
  if (hint) node.append(mn("small", "", hint)); return node;
}
function mi(id, value, maxLength) {
  const input = mn("input"); input.id = id; input.value = value; input.maxLength = maxLength; return input;
}
function isManagement() { return MANAGEMENT_SECTIONS.includes(state.section); }
function initManagement() {
  const nav = document.querySelector(".primary-nav");
  const managementNav = nav.querySelector('[data-nav-group="management"]');
  managementNav.hidden = true;
  const menuKeys = ["accountMenu", "groupMenu", "accessMenu", "viewMenu", "operationsMenu"];
  MANAGEMENT_SECTIONS.forEach((section, index) => {
    const link = mn("a", "nav-item"); link.href = `#${section}`; link.dataset.section = section;
    const icon = mn("span", "management-nav-icon", ["◎", "▦", "◇", "⌘", "◉"][index]); icon.setAttribute("aria-hidden", "true");
    link.append(icon, mn("span", "management-nav-label", mt(menuKeys[index]))); managementNav.append(link);
    const page = mn("section", "management-page"); page.id = `${section}-page`; page.hidden = true;
    document.querySelector(".page-content").append(page);
  });
  const mobile = mn("select", "mobile-page-select"); mobile.id = "mobile-page-select";
  for (const group of nav.querySelectorAll(".nav-group")) {
    const options = document.createElement("optgroup"); options.label = group.querySelector(".nav-group-label").textContent.trim(); options.dataset.navGroup = group.dataset.navGroup;
    for (const link of group.querySelectorAll("a")) options.append(new Option(link.textContent.trim(), link.dataset.navScope === "repository" ? `repository:${link.dataset.section}` : link.dataset.section));
    mobile.append(options);
  }
  mobile.addEventListener("change", () => { const local = mobile.value.startsWith("repository:"); navigateRepositorySection(local ? mobile.value.slice(11) : mobile.value, local ? selectedRepository() : ""); });
  document.querySelector(".page-content").prepend(mobile);
  window.addEventListener("beforeunload", (event) => { if (management.dirty) { event.preventDefault(); event.returnValue = ""; } });
}
function setManagementVisibility(visible) {
  const managementNav = document.querySelector('[data-nav-group="management"]');
  managementNav.hidden = !visible;
  for (const section of MANAGEMENT_SECTIONS) {
    document.querySelector(`#mobile-page-select option[value="${section}"]`).hidden = !visible;
  }
  if (!visible) {
    management.request++;
    if (activeGroupEditor) { const dialog = activeGroupEditor.dialog; activeGroupEditor = null; management.dirty = false; dialog.close(); }
    if (activeGroupTransfer) { const dialog = activeGroupTransfer.dialog; activeGroupTransfer = null; management.dirty = false; dialog.close(); }
    groupBrowser.filter = "all";
    document.getElementById("account-groups-page").replaceChildren();
    if (repositoryAccessEditor.drafts.size) management.dirty = false;
    resetRepositoryAccessEditor();
    management.repositoryAccess = { repositories: [], groups: [] };
    document.getElementById("repository-access-page").replaceChildren();
    resetOperationsState();
    document.getElementById("operations-page").replaceChildren();
  }
}
function managementLocale() {
  const menuKeys = ["accountMenu", "groupMenu", "accessMenu", "viewMenu", "operationsMenu"];
  MANAGEMENT_SECTIONS.forEach((section, index) => {
    const label = mt(menuKeys[index]);
    document.querySelector(`[data-section="${section}"] .management-nav-label`).textContent = label;
    document.querySelector(`#mobile-page-select option[value="${section}"]`).textContent = label;
  });
  const mobile = document.getElementById("mobile-page-select");
  mobile.setAttribute("aria-label", mt("navigation"));
  for (const options of mobile.querySelectorAll("optgroup")) {
    options.label = document.querySelector(`.nav-group[data-nav-group="${options.dataset.navGroup}"] .nav-group-label`).textContent.trim();
  }
  for (const link of document.querySelectorAll(".primary-nav a[data-section]")) {
    const option = [...mobile.options].find(item => item.value === (link.dataset.navScope === "repository" ? `repository:${link.dataset.section}` : link.dataset.section));
    if (option) option.textContent = (link.querySelector(".management-nav-label") || link.querySelector("span")).textContent;
  }
  // Preserve authored drafts when changing language.
  if (isManagement() && (!management.dirty || state.section === "repository-access")) renderManagement();
}
function managementShell(section, titleKey, introKey) {
  const page = document.getElementById(`${section}-page`); page.replaceChildren();
  const heading = mn("header", "page-heading"); const copy = mn("div");
  copy.append(mn("h1", "", mt(titleKey)), mn("p", "", mt(introKey)));
  heading.append(copy); page.append(heading); return page;
}
async function loadManagement() {
  if (state.section === "operations") return loadOperations();
  const serial = ++management.request;
  const section = state.section;
  const page = document.getElementById(`${section}-page`);
  if (!management.loaded) { page.replaceChildren(mn("p", "management-note", mt("loading"))); }
  try {
    const [accounts, groups, repositories] = await Promise.all([api("/api/v1/accounts"), api("/api/v1/account-groups"), api("/api/v1/workspace-repositories")]);
    if (serial !== management.request) return;
    Object.assign(management, { accounts, groups, repositories, loaded: true });
    const repositoryAccess = section === "repository-access" ? await api("/api/v1/repository-group-access") : management.repositoryAccess;
    const views = section === "workspace-views" ? await api("/api/v1/sparse-views") : management.views;
    if (serial !== management.request) return;
    if (section === "repository-access") management.repositoryAccess = repositoryAccess;
    if (section === "workspace-views") management.views = views;
    if (state.section === section && !management.dirty) renderManagement();
  } catch (error) {
    if (serial !== management.request || state.section !== section) return;
    toast(error.message, "error");
    if (!management.dirty) page.replaceChildren(mn("p", "management-note", error.message), mb(mt("retry"), () => void loadManagement()));
  }
}
function renderManagement() {
  if (state.section === "operations") return renderOperations();
  if (!management.loaded) return;
  if (state.section === "accounts") renderAccounts();
  else if (state.section === "account-groups") renderGroups();
  else if (state.section === "repository-access") renderRepositoryAccess();
  else if (state.section === "workspace-views") renderViews();
}
async function managementMutation(button, path, method, data, after) {
  const controls = [...(button.closest("form")?.querySelectorAll("input, select, textarea, button") || [button])];
  const disabled = controls.map(control => control.disabled);
  controls.forEach(control => { control.disabled = true; });
  try {
    await api(path, { method, headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() }, ...(data !== undefined ? { body: JSON.stringify(data) } : {}) });
    management.dirty = false; toast(mt("saved"), "success"); await after();
  } catch (error) { toast(error.message, "error"); }
  finally { controls.forEach((control, index) => { control.disabled = disabled[index]; }); }
}
function renderAccounts() {
  const page = managementShell("accounts", "accounts", "accountIntro");
  const form = mn("form", "management-profile pipeline-panel");
  const name = mi("account-display-name", state.user.name || "", 100); name.required = true;
  const email = mi("account-email", state.user.email, 320); email.readOnly = true;
  const save = mb(mt("save"), () => {} , "primary"); save.type = "submit";
  form.append(mn("h2", "", mt("profile")), field(mt("name"), name), field(mt("email"), email), save);
  name.addEventListener("input", () => { management.dirty = true; });
  form.addEventListener("submit", event => {
    event.preventDefault();
    void managementMutation(save, "/api/v1/accounts/me", "POST", { name: name.value }, async () => {
      state.user = await api("/api/v1/me"); renderUser(state.user); await loadManagement();
    });
  });
  page.append(form, mn("p", "management-notice", mt("roleHint")));
  const panel = mn("section", "pipeline-panel"); const tableWrap = mn("div", "table-wrap"); const table = mn("table", "management-table");
  const head = mn("tr"); ["accounts", "email", "role", "joined", "login"].forEach(key => head.append(mn("th", "", mt(key))));
  const thead = mn("thead"); thead.append(head); const body = mn("tbody");
  const accounts = management.accounts.filter(a => `${a.name || ""} ${a.email}`.toLowerCase().includes(state.query));
  const canManageRoles = state.user.role === "admin";
  const administratorCount = management.accounts.filter(account => account.role === "admin").length;
  for (const account of accounts) {
    const row = mn("tr"); const identity = mn("td"); const person = mn("div", "management-person");
    person.append(mn("span", "user-avatar", initials(account.name || account.email)), mn("strong", "", account.name || account.email.split("@")[0]));
    if (account.id === state.user.id) person.append(mn("span", "os-badge", mt("me")));
    identity.append(person);
    const roleCell = mn("td");
    if (canManageRoles) {
      const controls = mn("form", "management-role-controls");
      const select = mn("select");
      select.add(new Option(mt("roleUser"), "user"));
      select.add(new Option(mt("roleAdmin"), "admin"));
      select.value = account.role;
      if (account.role === "admin" && administratorCount === 1) select.options[0].disabled = true;
      const saveRole = mb(mt("save"), () => {}, "primary"); saveRole.type = "submit"; saveRole.disabled = true;
      select.addEventListener("change", () => { saveRole.disabled = select.value === account.role; });
      controls.addEventListener("submit", event => {
        event.preventDefault();
        void managementMutation(saveRole, `/api/v1/accounts/${account.id}/role`, "POST", { role: select.value }, async () => {
          state.user = await api("/api/v1/me"); renderUser(state.user); await loadManagement();
        });
      });
      controls.append(select, saveRole); roleCell.append(controls);
    } else {
      roleCell.append(mn("span", "os-badge", mt(account.role === "admin" ? "roleAdmin" : "roleUser")));
    }
    row.append(identity, mn("td", "", account.email), roleCell, mn("td", "", formatDate(account.created_at)), mn("td", "", formatDate(account.last_login_at))); body.append(row);
  }
  table.append(thead, body); tableWrap.append(table); panel.append(tableWrap);
  if (!accounts.length) panel.append(mn("p", "management-note", mt("emptyAccounts")));
  page.append(panel);
}
const groupBrowser = { filter: "all" };
let activeGroupEditor = null;
let activeGroupTransfer = null;
Object.assign(managementCopy, {
  groupOwned: ["내가 소유한 그룹", "Groups you own", "我拥有的组"],
  groupJoined: ["내가 참여한 그룹", "Groups you belong to", "我加入的组"],
  groupAll: ["전체 그룹", "All groups", "所有组"],
  groupPeople: ["그룹에 속한 계정", "Accounts in groups", "组内账号"],
  groupSearch: ["그룹·소유자·구성원 검색…", "Search groups, owners or members…", "搜索组、所有者或成员…"],
  groupNoResults: ["검색 조건에 맞는 그룹이 없습니다.", "No groups match these filters.", "没有符合筛选条件的组。"],
  groupClear: ["검색 및 필터 초기화", "Clear search and filters", "清除搜索和筛选"],
  groupViewMembers: ["전체 구성원 보기", "View all members", "查看所有成员"],
  groupEdit: ["그룹 수정", "Edit group", "编辑组"],
  groupNoDescription: ["설명이 없습니다.", "No description yet.", "暂无描述。"],
  groupReadOnly: ["이 그룹은 소유자만 수정·삭제하거나 소유권을 이전할 수 있습니다.", "Only the owner can edit, delete or transfer this group.", "只有所有者可以编辑、删除或转让此组。"],
  groupMemberSearch: ["구성원 이름 또는 이메일 검색", "Search member names or emails", "搜索成员姓名或邮箱"],
  groupSelectedOnly: ["선택한 계정만 보기", "Show selected accounts only", "仅显示已选账号"],
  groupSelected: ["선택한 구성원", "Selected members", "已选成员"],
  groupOwnerRequired: ["소유자 · 필수", "Owner · required", "所有者 · 必选"],
  groupAdd: ["추가할 구성원", "Members to add", "要添加的成员"],
  groupRemove: ["제외할 구성원", "Members to remove", "要移除的成员"],
  groupUnchanged: ["구성원 변경이 없습니다.", "Membership is unchanged.", "成员未更改。"],
  groupLimit: ["소유자를 포함해 최대 200명까지 선택할 수 있습니다.", "Select up to 200 members, including the owner.", "最多选择 200 名成员（含所有者）。"],
  groupRemovalHint: ["제외한 구성원은 이 그룹을 통해 받은 Repository 접근 권한을 잃습니다. 다른 그룹이나 소유자·관리자 권한은 유지됩니다.", "Removed members lose repository access granted through this group. Access from other groups, ownership or administrator privileges remains.", "移除成员将撤销通过此组获得的仓库访问权限，其他组、所有者或管理员权限仍保留。"],
  groupSavingWait: ["그룹을 저장하고 있습니다. 완료 후 닫아주세요.", "The group is being saved. Wait before closing.", "正在保存组，请完成后再关闭。"],
  groupNameRequired: ["그룹 이름을 입력하세요.", "Enter a group name.", "请输入组名称。"],
  groupTransfer: ["소유권 이전", "Transfer ownership", "转让所有权"],
  groupNewOwner: ["새 소유자", "New owner", "新所有者"],
  groupTransferHint: ["현재 그룹 구성원인 관리자에게 소유권을 넘깁니다. 이전 소유자는 구성원으로 남고 그룹 수정·삭제 권한은 새 소유자에게 넘어갑니다.", "Transfer to an administrator who is already a group member. The previous owner remains a member; the new owner can edit and delete the group.", "将所有权转给已加入此组的管理员。原所有者仍是成员，新所有者可编辑和删除此组。"],
  groupTransferEmpty: ["이전할 관리자 구성원이 없습니다. 계정 관리에서 관리자를 지정하고 이 그룹에 추가하세요.", "No administrator member is available. Assign an administrator in Accounts and add them to this group first.", "没有可转让的管理员成员。请先在账号管理中指定管理员并将其加入此组。"],
  groupTransferConfirm: ["소유권 이전 확인", "Confirm transfer", "确认转让"],
  groupTransferred: ["그룹 소유권을 이전했습니다.", "Group ownership transferred.", "组所有权已转让。"],
});
function accountGroupMatches(group, query, accounts = new Map(management.accounts.map(account => [account.id, account]))) {
  const people = [...new Set([group.owner_id, ...group.member_ids])].map(id => {
    const account = accounts.get(id);
    return account ? `${account.name} ${account.email}` : id;
  });
  return [group.name, group.description, ...people].join(" ").toLocaleLowerCase().includes(query.trim().toLocaleLowerCase());
}
function renderGroups() {
  if (state.user?.role !== "admin") { document.getElementById("account-groups-page").replaceChildren(); return; }
  const page = managementShell("account-groups", "groups", "groupIntro");
  page.querySelector(".page-heading").append(mb(mt("newGroup"), () => openGroupEditor(), "primary"));
  const accounts = new Map(management.accounts.map(account => [account.id, account]));
  const accountName = id => accounts.get(id)?.name || accounts.get(id)?.email || id;
  const summary = mn("div", "group-summary");
  for (const [key, count] of [["groupAll", management.groups.length], ["groupOwned", management.groups.filter(group => group.owner_id === state.user.id).length], ["groupPeople", new Set(management.groups.flatMap(group => group.member_ids)).size]]) {
    const card = mn("div"); card.append(mn("span", "", mt(key)), mn("strong", "", String(count))); summary.append(card);
  }
  const tools = mn("div", "group-tools");
  const search = mi("group-search", state.query || "", 256); search.type = "search"; search.placeholder = mt("groupSearch"); search.setAttribute("aria-label", mt("groupSearch"));
  const filters = mn("div", "group-filters"); filters.setAttribute("role", "group"); filters.setAttribute("aria-label", mt("groupSelect"));
  const list = mn("div", "management-group-grid");
  const count = mn("p", "management-note"); count.setAttribute("role", "status");
  const paint = () => {
    for (const button of filters.children) button.setAttribute("aria-pressed", String(button.dataset.filter === groupBrowser.filter));
    const groups = management.groups.filter(group => accountGroupMatches(group, state.query || "", accounts) && (groupBrowser.filter === "all" || (groupBrowser.filter === "owned" ? group.owner_id === state.user.id : group.member_ids.includes(state.user.id))));
    count.textContent = `${mt("groups")} · ${groups.length} / ${management.groups.length}`;
    list.replaceChildren();
    for (const group of groups) {
      const card = mn("article", "pipeline-panel management-group-card");
      const relationship = group.owner_id === state.user.id ? "owner" : group.member_ids.includes(state.user.id) ? "member" : "adminView";
      const header = mn("header", "management-card-heading");
      const identity = mn("div", "group-card-identity");
      const avatar = mn("span", "group-avatar", Array.from(group.name.trim())[0] || "#"); avatar.setAttribute("aria-hidden", "true");
      identity.append(avatar, mn("h2", "", group.name));
      header.append(identity, mn("span", `os-badge group-relationship group-relationship--${relationship}`, mt(relationship)));
      const owner = accounts.get(group.owner_id);
      const ownerLine = mn("p", "group-owner", `${mt("owner")} · ${accountName(group.owner_id)}`); ownerLine.title = owner?.email || group.owner_id;
      card.append(header, mn("p", "management-description", group.description || mt("groupNoDescription")), ownerLine);
      const memberHeading = mn("h3", "group-members-heading");
      memberHeading.append(mn("span", "", mt("members")), mn("span", "group-member-count", String(group.member_ids.length)));
      card.append(memberHeading);
      const chips = ids => {
        const members = mn("div", "management-member-chips");
        for (const id of ids) { const chip = mn("span", "management-chip", accountName(id)); chip.title = accounts.get(id)?.email || id; members.append(chip); }
        return members;
      };
      card.append(chips(group.member_ids.slice(0, 6)));
      if (group.member_ids.length > 6) {
        const more = mn("details", "group-more-members"); more.append(mn("summary", "", `${mt("groupViewMembers")} · ${group.member_ids.length}`), chips(group.member_ids.slice(6))); card.append(more);
      }
      if (group.owner_id === state.user.id) {
        const actions = mn("div", "management-actions group-card-actions");
        actions.append(mb(mt("groupEdit"), () => openGroupEditor(group)), mb(mt("groupTransfer"), () => openGroupOwnershipTransfer(group)), mb(mt("remove"), () => confirmManagement(mt("deleteGroup"), group.name, button => managementMutation(button, `/api/v1/account-groups/${group.id}`, "DELETE", undefined, async () => { document.getElementById("management-dialog").close(); await loadManagement(); })), "danger"));
        card.append(actions);
      } else card.append(mn("p", "management-note group-card-readonly", mt("groupReadOnly")));
      list.append(card);
    }
    if (!groups.length) {
      const empty = mn("div", "group-empty pipeline-panel"); empty.append(mn("p", "management-note", mt(management.groups.length ? "groupNoResults" : "emptyGroups")));
      if (management.groups.length) empty.append(mb(mt("groupClear"), () => { state.query = ""; search.value = ""; elements["pipeline-search"].value = ""; groupBrowser.filter = "all"; paint(); }));
      list.append(empty);
    }
  };
  for (const [value, key] of [["all", "groupAll"], ["owned", "groupOwned"], ["joined", "groupJoined"]]) {
    const button = mb(mt(key), () => { groupBrowser.filter = value; paint(); }); button.dataset.filter = value; filters.append(button);
  }
  search.addEventListener("input", () => { state.query = search.value; elements["pipeline-search"].value = search.value; paint(); });
  tools.append(search, filters); page.append(summary, tools, count, list, mn("p", "management-note", mt("groupHint"))); paint();
}
function managementDialog(title, requestClose) {
  document.getElementById("management-dialog")?.remove();
  const dialog = mn("dialog", "modal"); dialog.id = "management-dialog";
  const form = mn("form"); const header = mn("div", "modal-header"); const heading = mn("h2", "", title); heading.id = "management-dialog-title"; dialog.setAttribute("aria-labelledby", heading.id);
  header.append(heading, mb(mt("close"), () => requestClose ? requestClose() : dialog.close())); form.append(header); dialog.append(form); document.body.append(dialog); return { dialog, form };
}
function groupDraftChanges(group, name, description, selected) {
  const baseline = new Set(group?.member_ids || []);
  const added = [...selected].filter(id => !baseline.has(id)), removed = [...baseline].filter(id => !selected.has(id));
  return { added, removed, dirty: name !== (group?.name || "") || description !== (group?.description || "") || Boolean(group && (added.length || removed.length)) || (!group && selected.size > 1) };
}
function openGroupEditor(group) {
  if (state.user?.role !== "admin" || (group && group.owner_id !== state.user.id) || activeGroupEditor) return;
  const userId = state.user.id, ownerId = group?.owner_id || userId;
  const selected = new Set(group?.member_ids || []); selected.add(ownerId);
  let saving = false, saved = false;
  const requestClose = () => {
    if (saving) { toast(mt("groupSavingWait"), "error"); return false; }
    if (changes().dirty && !window.confirm(mt("discard"))) return false;
    management.dirty = false;
    dialog.close(); return true;
  };
  const { dialog, form } = managementDialog(group ? mt("groupEdit") : mt("newGroup"), requestClose);
  dialog.classList.add("group-editor");
  const editor = { dialog, requestClose, get saving() { return saving; } }; activeGroupEditor = editor;
  const current = () => activeGroupEditor === editor && state.user?.id === userId && state.user?.role === "admin";
  const name = mi("group-name", group?.name || "", 100); name.required = true;
  const description = mn("textarea"); description.id = "group-description"; description.value = group?.description || ""; description.maxLength = 500; description.rows = 3;
  const changes = () => groupDraftChanges(group, name.value, description.value, selected);
  form.append(field(mt("groupName"), name), field(mt("description"), description));
  const search = mi("group-member-search", "", 256); search.type = "search"; search.placeholder = mt("groupMemberSearch"); search.setAttribute("aria-label", mt("groupMemberSearch"));
  const selectedOnly = mn("input"); selectedOnly.type = "checkbox";
  const filter = mn("label", "group-selected-filter"); filter.append(selectedOnly, mn("span", "", mt("groupSelectedOnly")));
  const count = mn("p", "management-note"); count.setAttribute("role", "status");
  const members = mn("fieldset", "management-member-picker");
  const review = mn("div", "group-member-review"); review.setAttribute("role", "status");
  const error = mn("p", "group-editor-error"); error.setAttribute("role", "alert"); error.hidden = true;
  const actions = mn("div", "modal-actions"); const save = mb(mt("save"), () => {}, "primary"); save.type = "submit";
  actions.append(mb(mt("cancel"), requestClose), save);
  form.append(search, filter, count, members, review, mn("p", "management-note", mt("groupLimit")), error, actions);
  const accounts = new Map(management.accounts.map(account => [account.id, account]));
  for (const id of selected) if (!accounts.has(id)) accounts.set(id, { id, name: id, email: "" });
  const displayName = id => accounts.get(id)?.name || accounts.get(id)?.email || id;
  const paintChanges = () => {
    const diff = changes(); management.dirty = diff.dirty;
    count.textContent = `${mt("groupSelected")}: ${selected.size} / 200`;
    review.replaceChildren();
    for (const [key, ids] of [["groupAdd", diff.added], ["groupRemove", diff.removed]]) {
      if (!ids.length) continue;
      const row = mn("div"); row.append(mn("strong", "", `${mt(key)} · ${ids.length}`));
      const chips = mn("div", "management-member-chips");
      for (const id of ids) chips.append(mn("span", "management-chip", displayName(id)));
      row.append(chips); review.append(row);
    }
    if (!diff.added.length && !diff.removed.length) review.append(mn("p", "management-note", mt("groupUnchanged")));
    if (diff.removed.length) review.append(mn("p", "management-note", mt("groupRemovalHint")));
    if (selected.size > 200) review.append(mn("p", "group-editor-error", mt("groupLimit")));
    name.setCustomValidity(name.value && !name.value.trim() ? mt("groupNameRequired") : "");
    save.disabled = saving || !name.value.trim() || selected.size > 200 || Boolean(group && !diff.dirty);
    save.textContent = mt(saving ? "saving" : "save");
  };
  const paintMembers = () => {
    members.replaceChildren(mn("legend", "", mt("members")));
    const query = search.value.trim().toLocaleLowerCase();
    const visible = [...accounts.values()].filter(account => (!selectedOnly.checked || selected.has(account.id)) && `${account.name} ${account.email}`.toLocaleLowerCase().includes(query));
    for (const account of visible) {
      const label = mn("label", "management-member-option"); const checkbox = mn("input"); checkbox.type = "checkbox"; checkbox.value = account.id;
      checkbox.checked = selected.has(account.id); checkbox.disabled = saving || account.id === ownerId;
      const copy = mn("span", "group-member-identity"); copy.append(mn("strong", "", displayName(account.id)), mn("small", "", account.email));
      label.append(checkbox, copy);
      if (account.id === ownerId) label.append(mn("span", "group-owner-label", mt("groupOwnerRequired")));
      checkbox.addEventListener("change", () => {
        checkbox.checked ? selected.add(account.id) : selected.delete(account.id);
        error.hidden = true; paintChanges();
        if (selectedOnly.checked && !checkbox.checked) { paintMembers(); selectedOnly.focus(); }
      });
      members.append(label);
    }
    if (!visible.length) members.append(mn("p", "management-note", mt("emptyAccounts")));
  };
  name.addEventListener("input", () => { error.hidden = true; paintChanges(); });
  description.addEventListener("input", () => { error.hidden = true; paintChanges(); });
  search.addEventListener("input", paintMembers); selectedOnly.addEventListener("change", paintMembers);
  search.addEventListener("keydown", event => { if (event.key === "Enter") event.preventDefault(); });
  dialog.addEventListener("cancel", event => { event.preventDefault(); requestClose(); });
  dialog.addEventListener("close", () => {
    if (activeGroupEditor !== editor) return;
    activeGroupEditor = null; management.dirty = false;
    if (saved && state.section === "account-groups") renderGroups();
  });
  form.addEventListener("submit", async event => {
    event.preventDefault();
    if (saving || save.disabled || !current() || !form.reportValidity()) return;
    saving = true; error.hidden = true; dialog.setAttribute("aria-busy", "true");
    for (const control of form.querySelectorAll("input, textarea, button")) control.disabled = true;
    paintChanges();
    try {
      const result = await api(`/api/v1/account-groups${group ? `/${group.id}` : ""}`, { method: "POST", headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() }, body: JSON.stringify({ name: name.value.trim(), description: description.value.trim(), member_ids: [...selected] }) });
      if (!current()) return;
      management.request++;
      management.groups = [...management.groups.filter(item => item.id !== result.id), result].sort((a, b) => a.name.localeCompare(b.name, state.locale));
      saved = true; saving = false; toast(mt("saved"), "success"); dialog.close();
    } catch (failure) {
      if (current()) { error.textContent = failure.message; error.hidden = false; }
    } finally {
      if (current() && !saved) {
        saving = false; dialog.setAttribute("aria-busy", "false");
        for (const control of form.querySelectorAll("input, textarea, button")) control.disabled = false;
        paintMembers(); paintChanges();
      }
    }
  });
  paintMembers(); paintChanges(); dialog.showModal(); name.focus();
}
function eligibleGroupOwners(group) {
  const members = new Set(group.member_ids);
  return management.accounts.filter(account => account.id !== group.owner_id && members.has(account.id) && account.role === "admin");
}
function openGroupOwnershipTransfer(group) {
  if (state.user?.role !== "admin" || group.owner_id !== state.user.id || activeGroupEditor || activeGroupTransfer) return;
  const userId = state.user.id, candidates = eligibleGroupOwners(group);
  let saving = false;
  const requestClose = () => {
    if (saving) { toast(mt("groupSavingWait"), "error"); return false; }
    dialog.close(); return true;
  };
  const { dialog, form } = managementDialog(mt("groupTransfer"), requestClose);
  dialog.classList.add("group-transfer-dialog");
  const transfer = { dialog, requestClose }; activeGroupTransfer = transfer;
  const current = () => activeGroupTransfer === transfer && state.user?.id === userId && state.user?.role === "admin";
  form.append(mn("strong", "group-transfer-name", group.name), mn("p", "modal-intro", mt("groupTransferHint")));
  const select = mn("select"); select.id = "group-new-owner"; select.required = true;
  for (const account of candidates) {
    const option = mn("option", "", `${account.name || account.email} · ${account.email}`);
    option.value = account.id; select.append(option);
  }
  if (candidates.length) select.value = candidates[0].id;
  form.append(field(mt("groupNewOwner"), select));
  if (!candidates.length) form.append(mn("p", "management-note", mt("groupTransferEmpty")));
  const error = mn("p", "group-editor-error"); error.setAttribute("role", "alert"); error.hidden = true;
  const actions = mn("div", "modal-actions"), save = mb(mt("groupTransferConfirm"), () => {}, "primary"); save.type = "submit"; save.disabled = !candidates.length;
  actions.append(mb(mt("cancel"), requestClose), save); form.append(error, actions);
  dialog.addEventListener("cancel", event => { event.preventDefault(); requestClose(); });
  dialog.addEventListener("close", () => { if (activeGroupTransfer === transfer) activeGroupTransfer = null; });
  form.addEventListener("submit", async event => {
    event.preventDefault();
    if (saving || !candidates.length || !current() || !form.reportValidity()) return;
    saving = true; management.dirty = true; error.hidden = true; dialog.setAttribute("aria-busy", "true");
    for (const control of form.querySelectorAll("select, button")) control.disabled = true;
    save.textContent = mt("saving");
    try {
      const result = await api(`/api/v1/account-groups/${group.id}/owner`, { method: "POST", headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() }, body: JSON.stringify({ owner_id: select.value }) });
      if (!current()) return;
      management.request++;
      management.groups = management.groups.map(item => item.id === group.id ? result : item);
      management.dirty = false; saving = false;
      toast(mt("groupTransferred"), "success"); dialog.close();
      if (state.section === "account-groups") renderGroups();
    } catch (failure) {
      if (current()) { error.textContent = failure.message; error.hidden = false; }
    } finally {
      if (current() && saving) {
        saving = false; management.dirty = false; dialog.setAttribute("aria-busy", "false");
        for (const control of form.querySelectorAll("select, button")) control.disabled = false;
      }
    }
  });
  dialog.showModal(); if (candidates.length) select.focus();
}
function confirmManagement(message, name, action) {
  const { dialog, form } = managementDialog(mt("remove"));
  form.append(mn("strong", "", name), mn("p", "modal-intro", message));
  const actions = mn("div", "modal-actions"); const button = mb(mt("remove"), () => void action(button), "danger"); actions.append(mb(mt("cancel"), () => dialog.close()), button); form.append(actions);
  form.addEventListener("submit", event => event.preventDefault()); dialog.showModal();
}
function discardManagement() {
  if (activeGroupEditor && !activeGroupEditor.requestClose()) return false;
  if (activeGroupTransfer && !activeGroupTransfer.requestClose()) return false;
  if (repositoryAccessEditor.saving.size) { toast(mt("accessSavingWait"), "error"); return false; }
  if (management.dirty && !window.confirm(mt("discard"))) return false;
  resetRepositoryAccessEditor(); management.dirty = false; return true;
}
const repositoryAccessEditor = { selected: "", filter: "all", groupQuery: "", drafts: new Map(), saving: new Set(), errors: new Map(), generation: 0 };
Object.assign(managementCopy, {
  accessRepositories: ["저장소", "Repositories", "仓库"],
  accessConnected: ["그룹 연결됨", "Groups assigned", "已分配组"],
  accessUnconnected: ["그룹 연결 없음", "No groups assigned", "未分配组"],
  accessPending: ["변경한 저장소", "Repositories with changes", "有更改的仓库"],
  accessAll: ["전체", "All", "全部"],
  accessFilter: ["그룹 연결 상태", "Group assignment status", "组分配状态"],
  accessNoResults: ["검색 조건에 맞는 저장소가 없습니다.", "No repositories match these filters.", "没有符合筛选条件的仓库。"],
  accessClearFilters: ["검색 및 필터 초기화", "Clear search and filters", "清除搜索和筛选"],
  accessGroupSearch: ["그룹 이름 또는 설명 검색", "Search group names or descriptions", "搜索组名称或描述"],
  accessNoMatchingGroups: ["검색한 그룹이 없습니다. 다른 이름이나 설명으로 찾아보세요.", "No matching groups. Try another name or description.", "没有匹配的组，请尝试其他名称或描述。"],
  accessChoose: ["왼쪽 목록에서 권한을 편집할 저장소를 선택하세요.", "Choose a repository from the list to edit access.", "从列表中选择要编辑权限的仓库。"],
  accessSelectGroups: ["그룹별 접근 설정", "Group access settings", "组访问设置"],
  accessDraftHint: ["그룹을 선택하고 변경 내용을 확인한 뒤 저장하세요. 저장소를 전환해도 작성 중인 변경은 유지됩니다.", "Select groups, review the changes, then save. Drafts are kept when switching repositories.", "选择组、检查更改后保存。切换仓库时保留草稿。"],
  accessSelected: ["선택한 그룹", "Selected groups", "已选择的组"],
  accessAdd: ["추가할 그룹", "Groups to add", "要添加的组"],
  accessRemove: ["해제할 그룹", "Groups to remove", "要移除的组"],
  accessNoChanges: ["저장된 권한과 같습니다.", "Matches the saved access settings.", "与已保存的权限设置一致。"],
  accessDiscard: ["이 저장소 변경 취소", "Discard changes to this repository", "放弃此仓库的更改"],
  accessNoGrants: ["연결된 그룹이 없습니다. 저장소 소유자와 관리자의 기본 권한은 유지됩니다.", "No groups are assigned. Repository owners and administrators keep their access.", "未分配任何组，仓库所有者和管理员仍保留访问权限。"],
  accessRemovalHint: ["그룹 연결을 해제해도 다른 그룹이나 소유자·관리자 권한을 통한 접근은 유지됩니다.", "Removing a group does not remove access through other groups, ownership, or administrator privileges.", "移除组不会取消通过其他组、所有者或管理员权限获得的访问。"],
  accessLimit: ["저장소당 최대 200개 그룹을 연결할 수 있습니다.", "A repository supports up to 200 groups.", "每个仓库最多可分配 200 个组。"],
  accessSavingWait: ["권한을 저장하고 있습니다. 완료 후 이동하거나 새로고침하세요.", "Access is being saved. Wait before leaving or refreshing.", "正在保存权限，请完成后再离开或刷新。"],
  accessSaved: ["저장됨", "Saved", "已保存"],
});
function resetRepositoryAccessEditor() {
  repositoryAccessEditor.generation++;
  repositoryAccessEditor.drafts.clear(); repositoryAccessEditor.saving.clear(); repositoryAccessEditor.errors.clear();
  repositoryAccessEditor.selected = ""; repositoryAccessEditor.groupQuery = "";
  repositoryAccessEditor.filter = "all";
}
function repositoryAccessSelection(repository) {
  return repositoryAccessEditor.drafts.get(repository.resource_id) || new Set(repository.group_ids);
}
function repositoryAccessChanges(repository) {
  const selected = repositoryAccessSelection(repository), saved = new Set(repository.group_ids);
  return { added: [...selected].filter(id => !saved.has(id)), removed: [...saved].filter(id => !selected.has(id)) };
}
function updateRepositoryAccessDraft(repository, groupId, checked) {
  if (repositoryAccessEditor.saving.has(repository.resource_id)) return;
  const selected = new Set(repositoryAccessSelection(repository));
  checked ? selected.add(groupId) : selected.delete(groupId);
  repositoryAccessEditor.drafts.set(repository.resource_id, selected);
  const changes = repositoryAccessChanges(repository);
  if (!changes.added.length && !changes.removed.length) repositoryAccessEditor.drafts.delete(repository.resource_id);
  repositoryAccessEditor.errors.delete(repository.resource_id);
  management.dirty = repositoryAccessEditor.drafts.size > 0;
}
function filteredRepositoryAccess() {
  const query = (state.query || "").trim().toLocaleLowerCase();
  const names = new Map(management.repositoryAccess.groups.map(group => [group.id, group.name]));
  return management.repositoryAccess.repositories.filter(repository => {
    const ids = repositoryAccessSelection(repository);
    const text = [repository.repository_name, repository.resource_id, ...[...ids].map(id => names.get(id) || id)].join(" ").toLocaleLowerCase();
    return (!query || text.includes(query)) && (repositoryAccessEditor.filter === "all" || (repositoryAccessEditor.filter === "connected" ? ids.size > 0 : ids.size === 0));
  });
}
async function saveRepositoryAccess(resourceId) {
  const repository = management.repositoryAccess.repositories.find(item => item.resource_id === resourceId);
  if (!repository || state.user?.role !== "admin" || repositoryAccessEditor.saving.has(resourceId) || !repositoryAccessEditor.drafts.has(resourceId)) return;
  const groupIds = [...repositoryAccessSelection(repository)].sort();
  if (groupIds.length > 200) return;
  const generation = repositoryAccessEditor.generation, userId = state.user.id;
  const current = () => generation === repositoryAccessEditor.generation && state.user?.id === userId && state.user?.role === "admin";
  repositoryAccessEditor.saving.add(resourceId); repositoryAccessEditor.errors.delete(resourceId);
  renderRepositoryAccess();
  try {
    await api(`/api/v1/repository-group-access/${encodeURIComponent(resourceId)}`, {
      method: "POST", headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() }, body: JSON.stringify({ group_ids: groupIds }),
    });
    if (!current()) return;
    const saved = management.repositoryAccess.repositories.find(item => item.resource_id === resourceId);
    if (saved) saved.group_ids = groupIds;
    repositoryAccessEditor.drafts.delete(resourceId);
    toast(`${repository.repository_name} · ${mt("saved")}`, "success");
  } catch (error) {
    if (current()) repositoryAccessEditor.errors.set(resourceId, error.message);
  } finally {
    if (current()) {
      repositoryAccessEditor.saving.delete(resourceId);
      management.dirty = repositoryAccessEditor.drafts.size > 0;
      if (state.section === "repository-access") renderRepositoryAccess();
    }
  }
}
function renderRepositoryAccess() {
  const previous = document.getElementById("repository-access-page");
  if (state.user?.role !== "admin") { previous.replaceChildren(); return; }
  const focused = previous.contains(document.activeElement) ? document.activeElement : null;
  const focusId = focused?.id, cursor = focused?.selectionStart;
  const page = managementShell("repository-access", "repositoryAccess", "accessIntro");
  const data = management.repositoryAccess, editor = repositoryAccessEditor;
  if (!data.repositories.some(repository => repository.resource_id === editor.selected)) editor.selected = filteredRepositoryAccess()[0]?.resource_id || "";
  const summary = mn("div", "repository-access-summary"); page.append(summary);
  const paintSummary = () => {
    summary.replaceChildren();
    for (const [key, count] of [["accessRepositories", data.repositories.length], ["accessConnected", data.repositories.filter(repository => repositoryAccessSelection(repository).size).length], ["accessPending", editor.drafts.size]]) {
      const card = mn("div", "repository-access-stat"); card.append(mn("span", "", mt(key)), mn("strong", "", String(count))); summary.append(card);
    }
  };
  paintSummary();
  const layout = mn("div", "repository-access-layout"), sidebar = mn("section", "pipeline-panel repository-access-sidebar"), detail = mn("section", "pipeline-panel repository-access-detail");
  sidebar.setAttribute("aria-label", mt("accessRepositories")); detail.setAttribute("aria-label", mt("accessSelectGroups"));
  const tools = mn("div", "repository-access-tools");
  const search = mi("repository-access-search", state.query || "", 200); search.type = "search"; search.placeholder = mt("accessSearch"); search.setAttribute("aria-label", mt("accessSearch"));
  const filters = mn("div", "repository-access-filters"); filters.setAttribute("role", "group"); filters.setAttribute("aria-label", mt("accessFilter"));
  for (const [value, key] of [["all", "accessAll"], ["connected", "accessConnected"], ["unconnected", "accessUnconnected"]]) {
    const button = mb(mt(key), () => { editor.filter = value; renderRepositoryAccess(); });
    button.id = `repository-access-filter-${value}`;
    button.setAttribute("aria-pressed", String(editor.filter === value)); filters.append(button);
  }
  tools.append(search, filters);
  const list = mn("div", "repository-access-list"); sidebar.append(tools, list);
  const paintList = () => {
    const activeId = list.contains(document.activeElement) ? document.activeElement.id : "";
    const scrollTop = list.scrollTop;
    list.replaceChildren();
    const rows = filteredRepositoryAccess();
    for (const repository of rows) {
      const button = mb("", () => { editor.selected = repository.resource_id; editor.groupQuery = ""; paintList(); paintDetail(); });
      button.id = `repository-access-repository-${repository.resource_id}`;
      button.className = "repository-access-repository"; button.setAttribute("aria-pressed", String(editor.selected === repository.resource_id));
      const copy = mn("span", "repository-access-repository-copy");
      copy.append(mn("strong", "", repository.repository_name), mn("small", "", repository.resource_id));
      const count = repositoryAccessSelection(repository).size;
      copy.append(mn("span", "repository-access-meta", `${mt("grantedGroups")} · ${count}`));
      button.append(copy);
      if (editor.drafts.has(repository.resource_id)) button.append(mn("span", "repository-access-draft", mt(editor.saving.has(repository.resource_id) ? "saving" : "unsaved")));
      list.append(button);
    }
    if (!rows.length) {
      list.append(mn("p", "management-note", mt(data.repositories.length ? "accessNoResults" : "noManageableRepositories")));
      if (data.repositories.length) list.append(mb(mt("accessClearFilters"), () => { state.query = ""; elements["pipeline-search"].value = ""; editor.filter = "all"; renderRepositoryAccess(); }));
    }
    list.scrollTop = scrollTop;
    if (activeId) document.getElementById(activeId)?.focus({ preventScroll: true });
  };
  const paintDetail = () => {
    detail.replaceChildren();
    const repository = data.repositories.find(item => item.resource_id === editor.selected);
    if (!repository) { detail.append(mn("p", "management-note", mt("accessChoose"))); return; }
    const resourceId = repository.resource_id, saving = editor.saving.has(resourceId);
    detail.setAttribute("aria-busy", String(saving));
    const heading = mn("header", "repository-access-heading");
    heading.append(mn("span", "repository-access-eyebrow", mt("accessSelectGroups")), mn("h2", "", repository.repository_name), mn("code", "", resourceId), mn("p", "management-note", mt("accessDraftHint")));
    detail.append(heading);
    const searchGroups = mi("repository-access-group-search", editor.groupQuery, 200); searchGroups.type = "search"; searchGroups.placeholder = mt("accessGroupSearch"); searchGroups.setAttribute("aria-label", mt("accessGroupSearch"));
    const groups = mn("fieldset", "repository-access-group-list");
    const selectedCount = mn("p", "repository-access-selection-count");
    const paintGroups = () => {
      groups.replaceChildren(mn("legend", "", mt("grantedGroups")));
      const query = editor.groupQuery.trim().toLocaleLowerCase();
      const matching = data.groups.filter(group => {
        const meta = management.groups.find(item => item.id === group.id);
        return `${group.name} ${meta?.description || ""}`.toLocaleLowerCase().includes(query);
      });
      for (const group of matching) {
        const meta = management.groups.find(item => item.id === group.id);
        const label = mn("label", "repository-access-group"), checkbox = mn("input"); checkbox.type = "checkbox"; checkbox.value = group.id;
        checkbox.id = `repository-access-group-${group.id}`;
        checkbox.checked = repositoryAccessSelection(repository).has(group.id); checkbox.disabled = saving;
        const copy = mn("span", "repository-access-group-copy"); copy.append(mn("strong", "", group.name));
        if (meta?.description) copy.append(mn("small", "", meta.description));
        label.append(checkbox, copy);
        if (meta) label.append(mn("span", "repository-access-meta", `${mt("members")} ${meta.member_ids.length}`));
        checkbox.addEventListener("change", () => { updateRepositoryAccessDraft(repository, group.id, checkbox.checked); paintChanges(); paintList(); paintSummary(); });
        groups.append(label);
      }
      if (!matching.length) groups.append(mn("p", "management-note", mt(data.groups.length ? "accessNoMatchingGroups" : "noGroup")));
    };
    searchGroups.addEventListener("input", () => { editor.groupQuery = searchGroups.value; paintGroups(); });
    const changes = mn("div", "repository-access-changes"); changes.setAttribute("role", "status");
    const error = mn("p", "repository-access-error", editor.errors.get(resourceId) || ""); error.setAttribute("role", "alert"); error.hidden = !editor.errors.has(resourceId);
    const actions = mn("footer", "repository-access-actions");
    const status = mn("span", "repository-access-meta");
    const cancel = mb(mt("accessDiscard"), () => { editor.drafts.delete(resourceId); editor.errors.delete(resourceId); management.dirty = editor.drafts.size > 0; paintDetail(); paintList(); paintSummary(); });
    const save = mb(mt(saving ? "saving" : "saveAccess"), () => void saveRepositoryAccess(resourceId), "primary");
    actions.append(status, cancel, save);
    const paintChanges = () => {
      const selected = repositoryAccessSelection(repository), diff = repositoryAccessChanges(repository), dirty = editor.drafts.has(resourceId);
      selectedCount.textContent = `${mt("accessSelected")}: ${selected.size} / ${data.groups.length}`;
      changes.replaceChildren();
      for (const [key, ids, style] of [["accessAdd", diff.added, "add"], ["accessRemove", diff.removed, "remove"]]) {
        if (!ids.length) continue;
        const row = mn("div", `repository-access-change repository-access-change--${style}`); row.append(mn("strong", "", `${mt(key)} · ${ids.length}`));
        const names = mn("div", "management-member-chips");
        for (const id of ids) names.append(mn("span", "management-chip", data.groups.find(group => group.id === id)?.name || id));
        row.append(names); changes.append(row);
      }
      if (!dirty) changes.append(mn("p", "management-note", mt("accessNoChanges")));
      if (!selected.size) changes.append(mn("p", "management-note", mt("accessNoGrants")));
      if (diff.removed.length) changes.append(mn("p", "management-note", mt("accessRemovalHint")));
      if (selected.size > 200) changes.append(mn("p", "repository-access-error", mt("accessLimit")));
      status.textContent = mt(saving ? "saving" : dirty ? "unsaved" : "accessSaved");
      save.disabled = saving || !dirty || selected.size > 200; cancel.disabled = saving || !dirty;
      error.hidden = !editor.errors.has(resourceId);
    };
    const body = mn("div", "repository-access-body"); body.append(searchGroups, selectedCount, groups, changes, error);
    detail.append(body, actions); paintGroups(); paintChanges();
  };
  search.addEventListener("input", () => { state.query = search.value; elements["pipeline-search"].value = search.value; paintList(); });
  layout.append(sidebar, detail); page.append(layout, mn("p", "repository-access-policy", mt("accessHint")));
  paintList(); paintDetail();
  if (focusId) {
    const input = document.getElementById(focusId); input?.focus({ preventScroll: true });
    if (Number.isInteger(cursor) && input?.setSelectionRange) input.setSelectionRange(cursor, cursor);
  }
}

function renderViews() {
  const page = managementShell("workspace-views", "views", "viewIntro");
  if (management.repositories.length) page.querySelector(".page-heading").append(mb(mt("newView"), () => openViewEditor(), "primary"));
  page.append(mn("p", "management-notice", mt("presetHint")));
  renderViewLibrary(page);
}
function renderViewLibrary(page) {
  const panel = mn("section", "pipeline-panel management-view-library");
  const header = mn("header", "panel-header"); const heading = mn("div"); heading.append(mn("h2", "", mt("viewList")), mn("p", "", mt("viewIntro"))); header.append(heading, mn("span", "repository-total", String(management.views.length))); panel.append(header);
  const query = state.query.toLowerCase();
  const views = management.views.filter(view => !query || `${view.name} ${view.repository_name} ${view.mode}`.toLowerCase().includes(query));
  const list = mn("div", "management-view-grid");
  for (const view of views) {
    const card = mn("article", "management-view-card");
    const heading = mn("header", "management-card-heading"); heading.append(mn("h2", "", view.name), mn("span", "os-badge", view.repository_name));
    const administratorAccess = state.user.role === "admin" && view.owner_id !== state.user.id ? ` · ${mt("adminView")}` : "";
    const details = mn("p", "management-description", `${mt(view.mode === "sparse" ? "sparse" : "full")} · ${formatDate(view.updated_at)}${administratorAccess}`);
    const actions = mn("div", "management-actions"); actions.append(mb(mt("download"), () => downloadSparseView(view)));
    if (view.can_manage) actions.append(mb(mt("edit"), () => openViewEditor(view)), mb(mt("remove"), () => confirmManagement(mt("deleteView"), view.name, button => managementMutation(button, `/api/v1/sparse-views/${view.id}`, "DELETE", undefined, async () => { document.getElementById("management-dialog").close(); await loadManagement(); })), "danger"));
    card.append(heading, details, actions); list.append(card);
  }
  if (!views.length) list.append(mn("p", "management-note", mt("emptyViews")));
  panel.append(list); page.append(panel);
}
function openViewEditor(view) {
  const { dialog, form } = managementDialog(view ? mt("editView") : mt("newView"));
  const name = mi("sparse-view-name", view?.name || "", 100); name.required = true;
  const repository = mn("select"); repository.required = true;
  management.repositories.forEach(item => repository.add(new Option(item.name, item.resource_id)));
  repository.value = view?.resource_id || management.repositories[0]?.resource_id || ""; repository.disabled = Boolean(view);
  const mode = mn("select"); mode.add(new Option(mt("sparse"), "sparse")); mode.add(new Option(mt("full"), "full")); mode.value = view?.mode || "sparse";
  const rules = mn("textarea", "management-rules"); rules.rows = 10; rules.spellcheck = false; rules.value = view?.rules ?? "**\n!/src/\n!/README.md\n";
  const rulesField = field(mt("rules"), rules, mt("ruleHint")); const fullHint = mn("p", "management-note", mt("fullHint"));
  const syncMode = () => { rulesField.hidden = mode.value === "full"; fullHint.hidden = mode.value !== "full"; }; syncMode(); mode.addEventListener("change", syncMode);
  form.append(field(mt("viewName"), name), field(mt("repoSelect"), repository), field(mt("mode"), mode), rulesField, fullHint);
  const actions = mn("div", "modal-actions"); const save = mb(mt("save"), () => {}, "primary"); save.type = "submit"; actions.append(mb(mt("cancel"), () => dialog.close()), save); form.append(actions);
  form.addEventListener("submit", event => {
    event.preventDefault();
    if (mode.value === "sparse" && (new TextEncoder().encode(rules.value).length > 10000 || !rules.value.split("\n").some(line => line.trim() && !line.trim().startsWith("#")))) { rules.setCustomValidity(mt("invalidRules")); rules.reportValidity(); return; }
    const payload = { name: name.value, mode: mode.value, rules: mode.value === "full" ? "" : rules.value };
    if (!view) payload.resource_id = repository.value;
    void managementMutation(save, `/api/v1/sparse-views${view ? `/${view.id}` : ""}`, "POST", payload, async () => { dialog.close(); await loadManagement(); });
  });
  dialog.showModal(); name.focus();
}
function downloadSparseView(view) {
  const contents = view.mode === "sparse" ? view.rules : "";
  const url = URL.createObjectURL(new Blob([contents], { type: "text/plain;charset=utf-8" }));
  const link = mn("a"); link.href = url; link.download = `${view.name.replace(/[^A-Za-z0-9._-]+/g, "-") || "view"}.view`; document.body.append(link); link.click(); link.remove(); window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}
