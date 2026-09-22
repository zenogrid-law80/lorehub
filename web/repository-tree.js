"use strict";

const repositoryTree = { request: 0, repository: "", branches: [], branch: "", revision: "", nodes: new Map(), entries: [], loading: false, error: "" };
const repositoryTreeCopy = {
  title: ["파일·폴더", "Files and folders", "文件与文件夹"],
  description: ["폴더를 클릭해 하위 노드를 펼치거나 접습니다. LINK는 연결된 폴더입니다.", "Click a folder to expand or collapse its child nodes. LINK marks a linked folder.", "点击文件夹展开或折叠子节点。LINK 表示链接文件夹。"],
  repository: ["저장소", "Repository", "仓库"], branch: ["브랜치", "Branch", "分支"],
  folder: ["일반 폴더", "Folder", "普通文件夹"], link: ["링크 폴더", "Linked folder", "链接文件夹"], file: ["파일", "File", "文件"],
  loading: ["하위 노드를 불러오는 중…", "Loading child nodes…", "正在加载子节点…"],
  empty: ["빈 폴더입니다.", "This folder is empty.", "此文件夹为空。"],
  noRepository: ["조회할 저장소가 없습니다.", "No repository available.", "没有可用仓库。"],
  noBranch: ["조회할 브랜치가 없습니다.", "No branches available.", "没有可用分支。"],
  retry: ["다시 시도", "Retry", "重试"],
};
function rtt(key) { return repositoryTreeCopy[key][state.locale === "ko" ? 0 : state.locale === "zh-CN" ? 2 : 1]; }
function treeNode(tag, text, className = "") {
  const node = document.createElement(tag); node.textContent = text; node.className = className; return node;
}
function treeButton(label, action) {
  const button = treeNode("button", label, "button button--ghost"); button.type = "button"; button.addEventListener("click", action); return button;
}
function treeCurrent(request, scope) { return request === repositoryTree.request && scope === state.repositoryScope && state.section === "repository-tree"; }
async function loadRepositoryTreePage() {
  const request = ++repositoryTree.request, scope = state.repositoryScope;
  Object.assign(repositoryTree, { loading: true, error: "", entries: [], branches: [], branch: "", revision: "", nodes: new Map() });
  renderRepositoryTree();
  try {
    const payload = await api("/api/v1/repositories");
    if (!treeCurrent(request, scope)) return;
    state.repositories = payload.repositories;
    const candidates = scopedRepositories();
    const repository = candidates.find(item => item.name === repositoryTree.repository) || candidates[0];
    repositoryTree.repository = repository?.name || "";
    if (!repository) return;
    const branches = await api(`/api/v1/repositories/${encodeURIComponent(repository.name)}/branches`);
    if (!treeCurrent(request, scope)) return;
    repositoryTree.branches = branches;
    const branch = selectRepositoryBranch(branches, repository.url);
    repositoryTree.branch = branch?.name || "";
    repositoryTree.revision = branch?.revision || "";
    if (branches.length) await loadRepositoryTreePath("");
  } catch (error) { if (treeCurrent(request, scope)) repositoryTree.error = error.message; }
  finally { if (treeCurrent(request, scope)) { repositoryTree.loading = false; renderRepositoryTree(); } }
}
async function loadRepositoryTreePath(path) {
  if (!path) {
    repositoryTree.request++;
    repositoryTree.nodes = new Map();
    repositoryTree.entries = [];
    repositoryTree.loading = true;
    repositoryTree.error = "";
  }
  let node = repositoryTree.nodes.get(path);
  if (!node) {
    node = { expanded: true, entries: [], loading: false, error: "", request: 0 };
    repositoryTree.nodes.set(path, node);
  }
  const generation = repositoryTree.request, scope = state.repositoryScope;
  const revision = repositoryTree.revision, repository = repositoryTree.repository;
  const request = ++node.request;
  Object.assign(node, { loading: true, error: "" });
  const current = () => treeCurrent(generation, scope) && repository === repositoryTree.repository && revision === repositoryTree.revision && repositoryTree.nodes.get(path) === node && node.request === request;
  renderRepositoryTree();
  try {
    const params = new URLSearchParams({ revision, path });
    const entries = await api(`/api/v1/repositories/${encodeURIComponent(repository)}/tree?${params}`);
    if (current()) {
      node.entries = entries;
      if (!path) repositoryTree.entries = entries;
    }
  } catch (error) {
    if (current()) { node.error = error.message; if (!path) repositoryTree.error = error.message; }
  } finally {
    if (current()) {
      node.loading = false;
      if (!path) repositoryTree.loading = false;
      renderRepositoryTree();
    }
  }
}
function toggleRepositoryTreeFolder(path) {
  const node = repositoryTree.nodes.get(path);
  if (node?.expanded) {
    node.expanded = false;
    renderRepositoryTree();
    return;
  }
  if (node) node.expanded = true;
  return loadRepositoryTreePath(path);
}
function renderRepositoryTreeNodes(entries, parentPath = "") {
  const list = treeNode("ul", "", "repository-tree-nodes");
  for (const entry of entries) {
    const path = parentPath ? `${parentPath}/${entry.name}` : entry.name;
    const item = treeNode("li", "", "repository-tree-item");
    if (entry.kind === "directory") {
      const node = repositoryTree.nodes.get(path);
      const button = treeButton("", () => void toggleRepositoryTreeFolder(path));
      button.className = "repository-tree-toggle";
      button.dataset.focusKey = `folder:${path}`;
      button.setAttribute("aria-expanded", String(Boolean(node?.expanded)));
      const icon = treeNode("span", node?.expanded ? "▾" : "▸", "repository-tree-icon"); icon.setAttribute("aria-hidden", "true");
      button.append(icon, treeNode("span", entry.name, "repository-tree-name"));
      if (entry.is_link) {
        const badge = treeNode("span", "LINK", "repository-tree-link"); badge.setAttribute("aria-label", rtt("link")); button.append(badge);
      }
      item.append(button);
      if (node?.expanded) {
        const children = treeNode("div", "", "repository-tree-children");
        children.setAttribute("aria-busy", String(node.loading));
        if (node.loading || node.error || !node.entries.length) {
          const message = treeNode("div", "", "repository-tree-message"); message.setAttribute("role", node.error ? "alert" : "status");
          message.append(treeNode("span", node.loading ? rtt("loading") : node.error || rtt("empty")));
          if (node.error) {
            const retry = treeButton(rtt("retry"), () => void loadRepositoryTreePath(path)); retry.dataset.focusKey = `retry:${path}`; message.append(retry);
          }
          children.append(message);
        } else children.append(renderRepositoryTreeNodes(node.entries, path));
        item.append(children);
      }
    } else {
      const row = treeNode("div", "", "repository-tree-file");
      const icon = treeNode("span", "·", "repository-tree-icon"); icon.setAttribute("aria-hidden", "true");
      row.append(icon, treeNode("span", entry.name, "repository-tree-name")); item.append(row);
    }
    list.append(item);
  }
  return list;
}
function renderRepositoryTree() {
  const page = document.getElementById("repository-tree-page");
  const focused = page.contains(document.activeElement) ? document.activeElement.dataset.focusKey : null;
  renderRepositoryTreePage(page);
  if (focused) {
    const controls = Array.from(page.querySelectorAll("[data-focus-key]"));
    // An async sibling load must not steal keyboard focus from another row.
    controls.find(control => control.dataset.focusKey === focused)?.focus({ preventScroll: true });
  }
}
function renderRepositoryTreePage(page) {
  page.replaceChildren();
  const heading = treeNode("div", "", "page-heading");
  const intro = treeNode("div", ""); intro.append(treeNode("h1", rtt("title")), treeNode("p", rtt("description"))); heading.append(intro); page.append(heading);
  const controls = treeNode("div", "", "repository-tree-controls");
  for (const [key, items, selected, change] of [
    ["repository", scopedRepositories().map(item => item.name), repositoryTree.repository, value => chooseRepositorySection("repository-tree", value)],
    ["branch", repositoryTree.branches.map(item => item.name), repositoryTree.branch, value => {
      const branch = repositoryTree.branches.find(item => item.name === value);
      if (!branch) return;
      repositoryTree.branch = branch.name; repositoryTree.revision = branch.revision; rememberRepositoryBranch(branch.name); void loadRepositoryTreePath("");
    }],
  ]) {
    const label = treeNode("label", rtt(key)); const select = document.createElement("select"); select.setAttribute("aria-label", rtt(key)); select.dataset.focusKey = key;
    for (const name of items) select.append(new Option(name, name));
    select.value = selected; select.disabled = !items.length;
    select.addEventListener("change", () => change(select.value)); label.append(select); controls.append(label);
  }
  page.append(controls);
  if (repositoryTree.revision) {
    const revision = treeNode("code", repositoryTree.revision.slice(0, 12), "repository-tree-revision"); revision.title = repositoryTree.revision; page.append(revision);
  }
  const content = treeNode("div", "", "repository-tree-content"); content.setAttribute("aria-live", "polite"); content.setAttribute("aria-busy", String(repositoryTree.loading)); page.append(content);
  if (repositoryTree.loading) { content.append(treeNode("p", rtt("loading"))); return; }
  if (repositoryTree.error) {
    const error = treeNode("p", repositoryTree.error); error.setAttribute("role", "alert"); content.append(error, treeButton(rtt("retry"), () => void (repositoryTree.revision ? loadRepositoryTreePath("") : loadRepositoryTreePage()))); return;
  }
  if (!repositoryTree.repository || !repositoryTree.revision || !repositoryTree.entries.length) {
    content.append(treeNode("p", rtt(!repositoryTree.repository ? "noRepository" : !repositoryTree.revision ? "noBranch" : "empty"))); return;
  }
  const list = renderRepositoryTreeNodes(repositoryTree.entries); list.setAttribute("aria-label", rtt("title")); content.append(list);
}
