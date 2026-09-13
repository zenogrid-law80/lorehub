"use strict";

const MANAGEMENT_SECTIONS = ["accounts", "account-groups", "workspace-views"];
const management = { accounts: [], groups: [], repositories: [], views: [], assignments: [], groupId: "", resourceId: "", loaded: false, request: 0, dirty: false };
const managementCopy = {
  accounts: ["계정 관리", "Accounts", "账号管理"], groups: ["계정 그룹 관리", "Account groups", "账号组管理"], views: ["Sparse Workspace View 설정", "Sparse Workspace Views", "稀疏工作区视图设置"],
  accountMenu: ["계정", "Accounts", "账号"], groupMenu: ["계정 그룹", "Account groups", "账号组"], viewMenu: ["Sparse View", "Sparse View", "稀疏视图"],
  accountIntro: ["조직 계정을 확인하고 관리자는 계정 등급을 변경할 수 있습니다.", "Browse organization accounts. Administrators can change account roles.", "查看组织账号。管理员可以更改账号等级。"],
  groupIntro: ["함께 작업할 계정을 그룹으로 묶고 구성원을 관리합니다. 관리자는 모든 그룹을 볼 수 있습니다.", "Organize accounts into groups and manage membership. Administrators can view every group.", "将账号整理为组并管理成员。管理员可以查看所有组。"],
  viewIntro: ["재사용 가능한 Sparse View를 만들고 그룹별로 선택합니다. 관리자는 모든 View를 볼 수 있습니다.", "Create reusable Sparse Views and select them for each group. Administrators can view every View.", "创建可复用的稀疏视图并为每个组进行选择。管理员可以查看所有视图。"],
  profile: ["내 프로필", "My profile", "我的资料"], name: ["표시 이름", "Display name", "显示名称"], email: ["이메일", "Email", "电子邮件"], login: ["최근 로그인", "Last sign-in", "最近登录"], joined: ["가입일", "Joined", "加入时间"],
  role: ["등급", "Role", "等级"], roleUser: ["일반 사용자", "Regular user", "普通用户"], roleAdmin: ["관리자", "Administrator", "管理员"], roleHint: ["관리자만 계정 등급을 변경할 수 있습니다. 마지막 관리자는 일반 사용자로 변경할 수 없습니다.", "Only administrators can change roles. The last administrator cannot be changed to a regular user.", "只有管理员可以更改等级。最后一名管理员不能更改为普通用户。"],
  save: ["저장", "Save", "保存"], saved: ["저장했습니다.", "Saved.", "已保存。"], saving: ["저장 중…", "Saving…", "正在保存…"], cancel: ["취소", "Cancel", "取消"], edit: ["수정", "Edit", "编辑"], remove: ["삭제", "Delete", "删除"], close: ["닫기", "Close", "关闭"],
  newGroup: ["새 그룹", "New group", "新建组"], groupName: ["그룹 이름", "Group name", "组名称"], description: ["설명", "Description", "描述"], members: ["구성원", "Members", "成员"], owner: ["소유자", "Owner", "所有者"], member: ["구성원", "Member", "成员"], adminView: ["관리자 열람", "Administrator view", "管理员查看"], me: ["나", "You", "您"],
  emptyAccounts: ["검색 결과가 없습니다.", "No matching accounts.", "没有匹配的账号。"], emptyGroups: ["표시할 그룹이 없습니다. 새 그룹을 만들어 시작하세요.", "No groups to display. Create a group to get started.", "暂无组。创建一个组以开始。"],
  groupHint: ["그룹 생성자만 이름과 구성원을 변경할 수 있습니다. 소유자는 항상 구성원으로 포함됩니다.", "Only the creator can edit the group. The owner is always a member.", "只有创建者可以编辑组。所有者始终是成员。"],
  groupSelect: ["계정 그룹", "Account group", "账号组"], repoSelect: ["리포지토리", "Repository", "仓库"], chooseGroup: ["그룹을 선택하세요", "Choose a group", "选择组"], chooseRepo: ["리포지토리를 선택하세요", "Choose a repository", "选择仓库"],
  noGroup: ["먼저 계정 그룹을 생성하거나 그룹에 참여하세요.", "Create or join an account group first.", "请先创建或加入账号组。"],
  noRepo: ["설정 가능한 리포지토리가 없습니다. 그룹 소유자는 본인 소유 리포지토리의 설정을 추가할 수 있습니다.", "No repositories available. Group owners can add presets for repositories they own.", "暂无可用仓库。组所有者可以为其拥有的仓库添加预设。"],
  full: ["전체 workspace", "Full workspace", "完整工作区"], sparse: ["Sparse workspace", "Sparse workspace", "稀疏工作区"], mode: ["Workspace 범위", "Workspace scope", "工作区范围"],
  rules: ["View 규칙", "View rules", "视图规则"], ruleHint: ["한 줄에 규칙 하나. 일반 패턴은 제외, ! 패턴은 포함입니다. 뒤의 규칙이 우선하며 #은 주석입니다.", "One rule per line. Patterns exclude; ! patterns include. Later rules win; # starts a comment.", "每行一条规则。普通模式排除，! 模式包含。后面的规则优先；# 表示注释。"],
  presetHint: ["Sparse View는 리포지토리별 재사용 프리셋입니다. 그룹은 목록에서 하나를 선택하며, 선택만으로 리포지토리 접근 권한이 부여되지는 않습니다.", "Sparse Views are reusable repository presets. Groups select one from the list; selection does not grant repository access.", "稀疏视图是可复用的仓库预设。组从列表中选择一个；选择不会授予仓库访问权限。"],
  fullHint: ["모든 경로를 포함합니다. 다운로드한 view 파일은 비어 있습니다.", "Includes all paths. The downloaded view file is empty.", "包含所有路径。下载的视图文件为空。"],
  download: ["view 파일 다운로드", "Download view file", "下载视图文件"], applyHint: ["다운로드한 파일은 새 clone에서 lore repository clone --view <파일> <URL>로 사용하세요.", "Use the downloaded file with lore repository clone --view <file> <URL> for a new clone.", "新克隆时使用 lore repository clone --view <文件> <URL>。"],
  unsaved: ["저장되지 않음", "Not saved", "未保存"], stored: ["저장된 설정", "Saved preset", "已保存预设"], readonly: ["그룹 구성원은 저장된 설정을 확인하고 다운로드할 수 있습니다.", "Group members can view and download saved presets.", "组成员可以查看和下载已保存的预设。"],
  deleteGroup: ["이 그룹과 Sparse View 선택 정보를 삭제할까요? Sparse View 목록과 계정, 리포지토리는 유지됩니다.", "Delete this group and its Sparse View selections? The Sparse View library, accounts, and repositories will be kept.", "删除此组及其稀疏视图选择？稀疏视图库、账号和仓库将保留。"],
  deleteView: ["이 Sparse View를 삭제할까요? 이 View를 사용한 모든 그룹의 선택도 해제됩니다.", "Delete this Sparse View? It will also be unselected from every group using it.", "删除此稀疏视图？使用它的所有组也将取消选择。"],
  discard: ["저장하지 않은 변경사항을 버릴까요?", "Discard unsaved changes?", "放弃未保存的更改？"],
  loading: ["불러오는 중…", "Loading…", "正在加载…"], retry: ["다시 시도", "Retry", "重试"],
  search: ["계정 또는 그룹 검색…", "Search accounts or groups…", "搜索账号或组…"], viewSearch: ["Sparse View 검색…", "Search Sparse Views…", "搜索稀疏视图…"], navigation: ["페이지 이동", "Go to page", "前往页面"],
  invalidRules: ["10,000바이트 이하의 규칙을 입력하세요. Sparse 모드는 주석 외 규칙이 필요합니다.", "Enter at most 10,000 bytes of rules. Sparse mode requires a non-comment rule.", "规则不能超过 10,000 字节。稀疏模式需要非注释规则。"],
  viewList: ["Sparse View 목록", "Sparse View library", "稀疏视图列表"], newView: ["새 Sparse View", "New Sparse View", "新建稀疏视图"], editView: ["Sparse View 수정", "Edit Sparse View", "编辑稀疏视图"], viewName: ["View 이름", "View name", "视图名称"],
  emptyViews: ["표시할 Sparse View가 없습니다.", "No Sparse Views to display.", "没有可显示的稀疏视图。"], assignments: ["그룹별 Sparse View 선택", "Sparse View selections by group", "按组选择稀疏视图"], assignmentHint: ["그룹 소유자는 리포지토리별로 목록의 View 하나를 선택할 수 있습니다.", "Group owners can select one listed View for each repository.", "组所有者可以为每个仓库选择一个列表中的视图。"],
  unassigned: ["선택 안 함", "Not selected", "未选择"], saveSelection: ["선택 저장", "Save selection", "保存选择"], createViewFirst: ["먼저 이 리포지토리의 Sparse View를 만드세요.", "Create a Sparse View for this repository first.", "请先为此仓库创建稀疏视图。"], noAssignedViews: ["이 그룹에 선택된 Sparse View가 없습니다.", "No Sparse Views are selected for this group.", "此组尚未选择稀疏视图。"]
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
  const menuKeys = ["accountMenu", "groupMenu", "viewMenu"];
  MANAGEMENT_SECTIONS.forEach((section, index) => {
    const link = mn("a", "nav-item"); link.href = `#${section}`; link.dataset.section = section;
    const icon = mn("span", "management-nav-icon", ["◎", "▦", "⌘"][index]); icon.setAttribute("aria-hidden", "true");
    link.append(icon, mn("span", "management-nav-label", mt(menuKeys[index]))); managementNav.append(link);
    const page = mn("section", "management-page"); page.id = `${section}-page`; page.hidden = true;
    document.querySelector(".page-content").append(page);
  });
  const mobile = mn("select", "mobile-page-select"); mobile.id = "mobile-page-select";
  for (const group of nav.querySelectorAll(".nav-group")) {
    const options = document.createElement("optgroup"); options.label = group.querySelector(".nav-group-label").textContent.trim(); options.dataset.navGroup = group.dataset.navGroup;
    for (const link of group.querySelectorAll("a")) options.append(new Option(link.textContent.trim(), link.dataset.section));
    mobile.append(options);
  }
  mobile.addEventListener("change", () => { window.location.hash = mobile.value; });
  document.querySelector(".page-content").prepend(mobile);
  window.addEventListener("beforeunload", (event) => { if (management.dirty) { event.preventDefault(); event.returnValue = ""; } });
}
function managementLocale() {
  const menuKeys = ["accountMenu", "groupMenu", "viewMenu"];
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
    const option = [...mobile.options].find(item => item.value === link.dataset.section);
    if (option) option.textContent = (link.querySelector(".management-nav-label") || link.querySelector("span")).textContent;
  }
  // Preserve authored drafts when changing language.
  if (isManagement() && !management.dirty) renderManagement();
}
function managementShell(section, titleKey, introKey) {
  const page = document.getElementById(`${section}-page`); page.replaceChildren();
  const heading = mn("header", "page-heading"); const copy = mn("div");
  copy.append(mn("p", "breadcrumb", "Zenogrid / Workspace"), mn("h1", "", mt(titleKey)), mn("p", "", mt(introKey)));
  heading.append(copy); page.append(heading); return page;
}
async function loadManagement() {
  const serial = ++management.request;
  const section = state.section;
  const page = document.getElementById(`${section}-page`);
  if (!management.loaded) { page.replaceChildren(mn("p", "management-note", mt("loading"))); }
  try {
    const [accounts, groups, repositories] = await Promise.all([api("/api/v1/accounts"), api("/api/v1/account-groups"), api("/api/v1/workspace-repositories")]);
    if (serial !== management.request) return;
    Object.assign(management, { accounts, groups, repositories, loaded: true });
    if (!groups.some(g => g.id === management.groupId)) management.groupId = groups[0]?.id || "";
    const views = section === "workspace-views" ? await api("/api/v1/sparse-views") : management.views;
    const assignments = section === "workspace-views" && management.groupId ? await api(`/api/v1/account-groups/${management.groupId}/views`) : [];
    if (serial !== management.request) return;
    if (section === "workspace-views") Object.assign(management, { views, assignments });
    if (state.section === section && !management.dirty) renderManagement();
  } catch (error) {
    if (serial !== management.request || state.section !== section) return;
    toast(error.message, "error");
    if (!management.dirty) page.replaceChildren(mn("p", "management-note", error.message), mb(mt("retry"), () => void loadManagement()));
  }
}
function renderManagement() {
  if (!management.loaded) return;
  if (state.section === "accounts") renderAccounts();
  else if (state.section === "account-groups") renderGroups();
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
function renderGroups() {
  const page = managementShell("account-groups", "groups", "groupIntro");
  page.querySelector(".page-heading").append(mb(mt("newGroup"), () => openGroupEditor(), "primary"));
  page.append(mn("p", "management-note", mt("groupHint")));
  const list = mn("div", "management-group-grid");
  const groups = management.groups.filter(g => `${g.name} ${g.description}`.toLowerCase().includes(state.query));
  for (const group of groups) {
    const card = mn("article", "pipeline-panel management-group-card");
    const relationship = group.owner_id === state.user.id ? "owner" : group.member_ids.includes(state.user.id) ? "member" : "adminView";
    const header = mn("header", "management-card-heading"); header.append(mn("h2", "", group.name), mn("span", "os-badge", mt(relationship)));
    card.append(header, mn("p", "management-description", group.description || "—"), mn("h3", "", `${mt("members")} · ${group.member_ids.length}`));
    const members = mn("div", "management-member-chips");
    for (const id of group.member_ids) { const account = management.accounts.find(a => a.id === id); const chip = mn("span", "management-chip", account?.name || account?.email || id); chip.title = account?.email || id; members.append(chip); }
    card.append(members); const actions = mn("div", "management-actions");
    actions.append(mb(mt("views"), () => { management.groupId = group.id; management.resourceId = ""; window.location.hash = "workspace-views"; }));
    if (group.owner_id === state.user.id) actions.append(mb(mt("edit"), () => openGroupEditor(group)), mb(mt("remove"), () => confirmManagement(mt("deleteGroup"), group.name, button => managementMutation(button, `/api/v1/account-groups/${group.id}`, "DELETE", undefined, async () => { document.getElementById("management-dialog").close(); await loadManagement(); })), "danger"));
    card.append(actions); list.append(card);
  }
  if (!groups.length) list.append(mn("p", "management-note", mt("emptyGroups"))); page.append(list);
}
function managementDialog(title) {
  document.getElementById("management-dialog")?.remove();
  const dialog = mn("dialog", "modal"); dialog.id = "management-dialog";
  const form = mn("form"); const header = mn("div", "modal-header"); const heading = mn("h2", "", title); heading.id = "management-dialog-title"; dialog.setAttribute("aria-labelledby", heading.id);
  header.append(heading, mb(mt("close"), () => dialog.close())); form.append(header); dialog.append(form); document.body.append(dialog); return { dialog, form };
}
function openGroupEditor(group) {
  const { dialog, form } = managementDialog(group ? mt("edit") : mt("newGroup"));
  const name = mi("group-name", group?.name || "", 100); name.required = true;
  const description = mi("group-description", group?.description || "", 500);
  form.append(field(mt("groupName"), name), field(mt("description"), description));
  const members = mn("fieldset", "management-member-picker"); members.append(mn("legend", "", mt("members")));
  for (const account of management.accounts) {
    const label = mn("label", "management-member-option"); const checkbox = mn("input"); checkbox.type = "checkbox"; checkbox.value = account.id;
    checkbox.checked = account.id === state.user.id || Boolean(group?.member_ids.includes(account.id)); checkbox.disabled = account.id === state.user.id;
    label.append(checkbox, mn("span", "", `${account.name || account.email} · ${account.email}`)); members.append(label);
  }
  form.append(members, mn("p", "management-note", mt("groupHint")));
  const actions = mn("div", "modal-actions"); const save = mb(mt("save"), () => {}, "primary"); save.type = "submit";
  actions.append(mb(mt("cancel"), () => dialog.close()), save); form.append(actions);
  form.addEventListener("submit", event => { event.preventDefault(); void managementMutation(save, `/api/v1/account-groups${group ? `/${group.id}` : ""}`, "POST", { name: name.value, description: description.value, member_ids: [...members.querySelectorAll("input:checked")].map(input => input.value) }, async () => { dialog.close(); await loadManagement(); }); });
  dialog.showModal(); name.focus();
}
function confirmManagement(message, name, action) {
  const { dialog, form } = managementDialog(mt("remove"));
  form.append(mn("strong", "", name), mn("p", "modal-intro", message));
  const actions = mn("div", "modal-actions"); const button = mb(mt("remove"), () => void action(button), "danger"); actions.append(mb(mt("cancel"), () => dialog.close()), button); form.append(actions);
  form.addEventListener("submit", event => event.preventDefault()); dialog.showModal();
}
function discardManagement() { if (management.dirty && !window.confirm(mt("discard"))) return false; management.dirty = false; return true; }
function renderViews() {
  const page = managementShell("workspace-views", "views", "viewIntro");
  if (management.repositories.length) page.querySelector(".page-heading").append(mb(mt("newView"), () => openViewEditor(), "primary"));
  page.append(mn("p", "management-notice", mt("presetHint")));
  renderViewLibrary(page);
  renderGroupViewAssignments(page);
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
function renderGroupViewAssignments(page) {
  const panel = mn("section", "pipeline-panel management-assignment-panel");
  const header = mn("header", "panel-header"); const heading = mn("div"); heading.append(mn("h2", "", mt("assignments")), mn("p", "", mt("assignmentHint"))); header.append(heading); panel.append(header);
  if (!management.groups.length) { panel.append(mn("p", "management-note", mt("noGroup"))); page.append(panel); return; }
  const groupSelect = mn("select"); groupSelect.id = "view-group";
  management.groups.forEach(group => groupSelect.add(new Option(group.name, group.id))); groupSelect.value = management.groupId;
  groupSelect.addEventListener("change", () => { management.groupId = groupSelect.value; management.assignments = []; panel.replaceChildren(mn("p", "management-note", mt("loading"))); void loadManagement(); });
  const picker = mn("div", "management-assignment-picker"); picker.append(field(mt("groupSelect"), groupSelect, mt("assignmentHint"))); panel.append(picker);
  const group = management.groups.find(item => item.id === management.groupId);
  const canManage = group?.owner_id === state.user.id;
  const repositories = new Map(management.assignments.map(item => [item.resource_id, { resource_id: item.resource_id, name: item.repository_name }]));
  if (canManage) management.repositories.forEach(repository => repositories.set(repository.resource_id, repository));
  const rows = mn("div", "management-assignment-list");
  for (const repository of [...repositories.values()].sort((a, b) => a.name.localeCompare(b.name))) {
    const current = management.assignments.find(item => item.resource_id === repository.resource_id);
    const row = mn("div", "management-assignment-row");
    const copy = mn("div", "management-assignment-copy"); copy.append(mn("strong", "", repository.name), mn("small", "", current ? current.view_name : mt("unassigned"))); row.append(copy);
    if (canManage) {
      const select = mn("select"); select.add(new Option(mt("unassigned"), ""));
      management.views.filter(view => view.can_manage && view.resource_id === repository.resource_id).forEach(view => select.add(new Option(view.name, view.id)));
      select.value = current?.view_id || "";
      const save = mb(mt("saveSelection"), () => {
        if (select.value) void managementMutation(save, `/api/v1/account-groups/${management.groupId}/views/${encodeURIComponent(repository.resource_id)}`, "POST", { view_id: select.value }, loadManagement);
        else if (current) void managementMutation(save, `/api/v1/account-groups/${management.groupId}/views/${encodeURIComponent(repository.resource_id)}`, "DELETE", undefined, loadManagement);
      }, "primary");
      save.disabled = select.value === (current?.view_id || ""); select.addEventListener("change", () => { save.disabled = select.value === (current?.view_id || ""); });
      const controls = mn("div", "management-assignment-controls"); controls.append(select, save); row.append(controls);
    } else if (current) row.append(mb(mt("download"), () => downloadSparseView(current)));
    rows.append(row);
  }
  if (!repositories.size) rows.append(mn("p", "management-note", canManage ? mt("createViewFirst") : mt("noAssignedViews")));
  panel.append(rows); page.append(panel);
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
