import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";

const source = await readFile(new URL("../web/app.js", import.meta.url), "utf8");
const treeSource = source.slice(source.indexOf("function repositoryIdentifier("), source.indexOf("function openNewRepositoryLink("));

function setup(api = async () => []) {
  const state = {
    locale: "en", repositoryLinksName: "game", repositoryLinksBranch: "main", repositoryLinksRevision: "a".repeat(64),
    repositoryLinks: [], repositories: [], repositoryLinkCollapsedPaths: new Set(), repositoryLinkOpenPaths: new Set(),
    repositoryLinkContent: new Map(), repositoryLinkSelectedPath: null, repositoryLinksStatus: "ready", repositoryLinksBusy: false,
  };
  const context = vm.createContext({ state, api, URLSearchParams, t: key => key });
  vm.runInContext(treeSource + "\nrenderRepositoryLinks = () => {};", context);
  return { state, context, get: expression => vm.runInContext(expression, context) };
}

test("the linked paths tree keeps Root visible without a right-side Add link button", () => {
  class Element {
    constructor() { this.children = []; this.dataset = {}; this.attributes = {}; this.listeners = {}; }
    append(...children) { this.children.push(...children); }
    setAttribute(name, value) { this.attributes[name] = value; }
    addEventListener(name, listener) { this.listeners[name] = listener; }
  }
  const { state, context, get } = setup();
  state.repositories = [{ name: "game" }, { name: "developer" }];
  context.document = { createElement: () => new Element() };
  context.textNode = (value, className) => Object.assign(new Element(), { textContent: value, className });
  context.openNewRepositoryLink = () => {};
  const root = get("renderRepositoryLinkRoot(new Map())");
  const item = root.children[0];
  const row = item.children[0];
  assert.equal(row.children[1].textContent, "game / main");
  assert.equal(row.children[2].textContent, "ROOT");
  assert.equal(row.children.length, 3);
  assert.equal(item.children[1].children[0].children[0].textContent, "Repository link가 없습니다");
  row.children[0].listeners.click();
  assert.ok(state.repositoryLinkCollapsedPaths.has("game\0main\0"));
});

test("linked nodes show status without a right-side actions menu", () => {
  class Element {
    constructor() { this.children = []; this.dataset = {}; }
    append(...children) { this.children.push(...children); }
    setAttribute() {}
    addEventListener() {}
  }
  const { state, context, get } = setup();
  state.repositoryLinks = [{ path: "Test", status: "current", auto_update: true }];
  context.document = { createElement: () => new Element() };
  context.textNode = (value, className) => Object.assign(new Element(), { textContent: value, className });
  const row = get("renderRepositoryLinkNodes(repositoryLinkTree(state.repositoryLinks).children)").children[0].children[0];
  assert.deepEqual(row.children.map(child => child.textContent), ["▸", "Test", "LINK", "Up to date"]);
});

test("folder context menu offers Add link and deletes only a real LINK", () => {
  class Element {
    constructor() { this.children = []; this.attributes = {}; this.listeners = {}; this.style = {}; }
    append(...children) { this.children.push(...children); }
    setAttribute(name, value) { this.attributes[name] = value; }
    addEventListener(name, listener) { this.listeners[name] = listener; }
    getBoundingClientRect() { return { left: 20, bottom: 30, width: 160, height: 90 }; }
    focus() {}
    remove() {}
  }
  const { state, context, get } = setup();
  const body = new Element();
  context.document = { createElement: () => new Element(), body };
  context.window = { innerWidth: 800, innerHeight: 600 };
  context.textNode = text => Object.assign(new Element(), { textContent: text });
  state.repositories = [{ name: "game" }, { name: "developer" }];
  state.repositoryLinks = [{ path: "assets/Test", auto_update: true }];
  let added;
  const performed = [];
  context.openNewRepositoryLink = path => { added = path; };
  context.performRepositoryLinkAction = (action, path) => { performed.push([action, path]); };
  let prevented = false;
  context.contextEvent = (path, kind = "directory") => ({ target: { closest: () => ({ dataset: { nodePath: path, nodeKind: kind }, getBoundingClientRect: () => ({ left: 20, bottom: 30 }) }) }, preventDefault() { prevented = true; }, clientX: 100, clientY: 100 });
  get("openRepositoryLinkContextMenu(contextEvent('assets'))");
  assert.equal(prevented, true);
  assert.equal(body.children[0].children[0].textContent, "Add link");
  assert.equal(body.children[0].children[1].disabled, true);
  assert.equal(body.children[0].children[2].textContent, "Only linked folders can be deleted here.");
  body.children[0].children[0].listeners.click();
  assert.equal(added, "assets");
  get("openRepositoryLinkContextMenu(contextEvent('assets/README.md', 'file'))");
  body.children.at(-1).children[0].listeners.click();
  assert.equal(added, "assets");
  get("openRepositoryLinkContextMenu(contextEvent('', 'root'))");
  assert.equal(body.children.at(-1).children.length, 1);
  get("openRepositoryLinkContextMenu(contextEvent('assets/Test', 'link'))");
  const linkMenu = body.children.at(-1);
  assert.equal(linkMenu.children.length, 4);
  assert.equal(linkMenu.children[1].textContent, "dynamic.updateLink");
  assert.equal(linkMenu.children[2].textContent, "Switch to manual sync");
  assert.equal(linkMenu.children[3].textContent, "Delete link");
  for (const button of linkMenu.children.slice(1)) button.listeners.click();
  assert.deepEqual(performed, [["update", "assets/Test"], ["policy", "assets/Test"], ["remove", "assets/Test"]]);
});

test("Add link preselects the same source folder for child nodes", () => {
  const formSource = source.slice(source.indexOf("function openNewRepositoryLink("), source.indexOf("async function loadRepositoryLinkSourceBranches("));
  const elements = Object.fromEntries([
    "repository-link-form", "repository-link-path", "repository-link-progress", "repository-link-root-name",
    "repository-link-source-repository", "repository-link-source-path", "create-repository-link-button",
    "new-repository-link-dialog",
  ].map(id => [id, { value: "", reset() {}, replaceChildren() {}, add() {}, showModal() {}, focus() {} }]));
  const state = {
    repositoryLinksBusy: false, repositoryLinksStatus: "ready", repositoryLinksRevision: "a".repeat(64),
    repositoryLinksName: "game", repositoryLinksBranch: "main",
    repositories: [{ name: "game" }, { name: "developer" }],
  };
  const context = vm.createContext({
    state, elements, Option: class { constructor(text, value) { this.text = text; this.value = value; } },
    t: key => key, setRepositoryLinkFormError() {}, loadRepositoryLinkSourceBranches() {},
    renderRepositoryLinkPreview() {}, window: { setTimeout() {} },
  });
  vm.runInContext(formSource, context);
  vm.runInContext("openNewRepositoryLink('assets/characters')", context);
  assert.equal(elements["repository-link-path"].value, "assets/characters/");
  assert.equal(elements["repository-link-source-path"].value, "assets/characters");
  vm.runInContext("openNewRepositoryLink()", context);
  assert.equal(elements["repository-link-path"].value, "");
  assert.equal(elements["repository-link-source-path"].value, ".");
});

test("links are grouped under their actual path segments", () => {
  const { get } = setup();
  const paths = get("repositoryLinkTree([{path:'assets/characters/hero'}, {path:'assets/environments'}, {path:'packages/shared'}])");
  assert.deepEqual([...paths.children.keys()], ["assets", "packages"]);
  assert.deepEqual([...paths.children.get("assets").children.keys()], ["characters", "environments"]);
  assert.equal(paths.children.get("assets").children.get("characters").children.get("hero").link.path, "assets/characters/hero");
});

test("linked folder loads children at its selected revision and keeps sibling state", async () => {
  const requests = [];
  const { state, get } = setup(async url => {
    requests.push(new URL(url, "http://test"));
    return [{ name: "child.txt", kind: "file" }];
  });
  get("toggleRepositoryLinkContent('assets/characters')");
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(requests[0].searchParams.get("path"), "assets/characters");
  assert.equal(requests[0].searchParams.get("revision"), "a".repeat(64));
  assert.equal(state.repositoryLinkContent.get("game\0main\0assets/characters").entries[0].name, "child.txt");
  get("toggleRepositoryLinkFolder('packages')");
  assert.ok(state.repositoryLinkCollapsedPaths.has("game\0main\0packages"));
  assert.ok(state.repositoryLinkOpenPaths.has("game\0main\0assets/characters"));
});

test("old folder responses cannot overwrite a new revision", async () => {
  let finish;
  const { state, get } = setup(() => new Promise(resolve => { finish = resolve; }));
  const pending = get("loadRepositoryLinkContent('shared')");
  state.repositoryLinksRevision = "b".repeat(64);
  state.repositoryLinkContent.clear();
  finish([{ name: "old.txt" }]);
  await pending;
  assert.equal(state.repositoryLinkContent.size, 0);
});

test("a refreshed folder ignores the earlier request even at the same revision", async () => {
  const finish = [];
  const { state, get } = setup(() => new Promise(resolve => finish.push(resolve)));
  const earlier = get("loadRepositoryLinkContent('shared')");
  state.repositoryLinkContent.clear();
  const refreshed = get("loadRepositoryLinkContent('shared')");
  finish[1]([{ name: "new.txt" }]);
  await refreshed;
  finish[0]([{ name: "old.txt" }]);
  await earlier;
  assert.equal(state.repositoryLinkContent.get("game\0main\0shared").entries[0].name, "new.txt");
});

test("context deletion uses the existing confirmed link removal request", async () => {
  const mutationSource = source.slice(source.indexOf("async function performRepositoryLinkAction("), source.indexOf("function openRepositoryConfig("));
  const calls = [];
  const state = {
    repositoryLinksBusy: false, repositoryLinksRevision: "a".repeat(64), repositoryLinksName: "game", repositoryLinksBranch: "main",
    repositoryLinks: [{ path: "assets/Test", auto_update: true }],
  };
  const context = vm.createContext({
    state, window: { confirm: message => { calls.push(["confirm", message]); return true; } },
    t: (key, values) => values?.path ? `${key}: ${values.path}` : key,
    api: async (path, options) => { calls.push(["api", path, JSON.parse(options.body)]); },
    csrfToken: () => "fixture-token", toast: () => {}, loadRepositoryLinks: async () => {}, renderRepositoryLinks: () => {},
  });
  vm.runInContext(mutationSource, context);
  const get = expression => vm.runInContext(expression, context);
  await get("performRepositoryLinkAction('remove', 'assets')");
  assert.equal(calls.length, 0);
  await get("performRepositoryLinkAction('remove', 'assets/Test')");
  assert.deepEqual(calls[0], ["confirm", "dynamic.removeLinkConfirm: assets/Test"]);
  assert.equal(calls[1][1], "/api/v1/repositories/game/links/remove");
  assert.deepEqual(calls[1][2], { branch: "main", expected_revision: "a".repeat(64), path: "assets/Test" });
  assert.equal(state.repositoryLinksBusy, false);
});
