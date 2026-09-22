import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";
const source = await readFile(new URL("../web/repository-tree.js", import.meta.url), "utf8");
function setup(api) {
  const context = vm.createContext({ URLSearchParams, api, state: { locale: "ko", section: "repository-tree", repositoryScope: "lores://host/game", repositories: [] }, scopedRepositories() { return context.state.repositories.filter(r => r.url === context.state.repositoryScope); } });
  vm.runInContext(source + "\nrenderRepositoryTree = () => {};", context);
  const get = code => vm.runInContext(code, context);
  get("Object.assign(repositoryTree, {repository:'game', revision:'a'.repeat(64)})");
  return { context, get };
}
test("expanding requests children at the selected revision; collapsing preserves sibling folders", async () => {
  const requests = [];
  const { get } = setup(async url => { requests.push(new URL(url, "http://test")); return [{ name: "child", kind: "directory", is_link: true }]; });
  await get("toggleRepositoryTreeFolder('Shared/한글 & #folder')");
  await get("toggleRepositoryTreeFolder('src')");
  assert.equal(requests[0].searchParams.get("path"), "Shared/한글 & #folder");
  assert.equal(requests[0].searchParams.get("revision"), "a".repeat(64));
  assert.equal(get("repositoryTree.nodes.get('src').entries[0].is_link"), true);
  get("toggleRepositoryTreeFolder('Shared/한글 & #folder')");
  assert.equal(get("repositoryTree.nodes.get('Shared/한글 & #folder').expanded"), false);
  assert.equal(get("repositoryTree.nodes.get('src').expanded"), true);
  assert.equal(requests.length, 2);
  await get("toggleRepositoryTreeFolder('Shared/한글 & #folder')");
  assert.equal(requests.length, 3);
});
test("simultaneous sibling requests finish independently without replacing the root", async () => {
  const finish = {};
  const { get } = setup(url => new Promise(resolve => { finish[new URL(url, "http://test").searchParams.get("path")] = resolve; }));
  get("repositoryTree.entries=[{name:'root-entry'}]");
  const first = get("toggleRepositoryTreeFolder('first')");
  const second = get("toggleRepositoryTreeFolder('second')");
  finish.second([{ name: "second-child" }]); await second;
  finish.first([{ name: "first-child" }]); await first;
  assert.equal(get("repositoryTree.nodes.get('first').entries[0].name"), "first-child");
  assert.equal(get("repositoryTree.nodes.get('second').entries[0].name"), "second-child");
  assert.equal(get("repositoryTree.entries[0].name"), "root-entry");
});
test("collapse during loading stays closed and a stale response cannot replace a reopened folder", async () => {
  const finish = [];
  const { get } = setup(() => new Promise(resolve => { finish.push(resolve); }));
  const old = get("toggleRepositoryTreeFolder('Shared')");
  get("toggleRepositoryTreeFolder('Shared')");
  assert.equal(get("repositoryTree.nodes.get('Shared').expanded"), false);
  const fresh = get("toggleRepositoryTreeFolder('Shared')");
  finish[1]([{ name: "fresh" }]); await fresh;
  finish[0]([{ name: "stale" }]); await old;
  assert.equal(get("repositoryTree.nodes.get('Shared').entries[0].name"), "fresh");
  get("toggleRepositoryTreeFolder('Shared')");
  assert.equal(get("repositoryTree.nodes.get('Shared').expanded"), false);
});
test("branch changes reset expanded nodes and ignore old folder requests", async () => {
  let old;
  const { get } = setup(url => url.includes("path=Shared") ? new Promise(resolve => { old = resolve; }) : Promise.resolve([{ name: "new-root" }]));
  const pending = get("toggleRepositoryTreeFolder('Shared')");
  get("repositoryTree.revision='b'.repeat(64)");
  await get("loadRepositoryTreePath('')");
  old([{ name: "obsolete" }]); await pending;
  assert.equal(get("repositoryTree.nodes.has('Shared')"), false);
  assert.equal(get("repositoryTree.entries[0].name"), "new-root");
});
test("repository switches and leaving the page discard pending results", async () => {
  for (const change of ["state.repositoryScope='lores://host/other'", "state.section='repositories'"]) {
    let complete;
    const { get } = setup(() => new Promise(resolve => { complete = resolve; }));
    const pending = get("toggleRepositoryTreeFolder('Shared')"); get(change); complete([{ name: "private" }]); await pending;
    assert.equal(get("repositoryTree.nodes.get('Shared').entries.length"), 0);
  }
});
test("errors and retry stay inside their folder without clearing siblings", async () => {
  let fail = true;
  const { get } = setup(async () => { if (fail) throw new Error("Access denied"); return []; });
  get("repositoryTree.entries=[{name:'previous'}]");
  await get("toggleRepositoryTreeFolder('Shared')");
  assert.equal(get("repositoryTree.nodes.get('Shared').error"), "Access denied");
  assert.equal(get("repositoryTree.nodes.get('Shared').loading"), false);
  assert.equal(get("repositoryTree.error"), "");
  assert.equal(get("repositoryTree.entries[0].name"), "previous");
  fail = false; await get("loadRepositoryTreePath('Shared')");
  assert.equal(get("repositoryTree.nodes.get('Shared').error"), "");
  assert.equal(get("repositoryTree.nodes.get('Shared').entries.length"), 0);
});
test("missing scoped repository never falls back to another repository", async () => {
  const calls = [];
  const { get } = setup(async url => { calls.push(url); return { repositories: [{ name: "other", url: "lores://host/other" }] }; });
  await get("loadRepositoryTreePage()");
  assert.deepEqual(calls, ["/api/v1/repositories"]);
  assert.equal(get("repositoryTree.repository"), "");
  assert.equal(get("repositoryTree.loading"), false);
});
test("page load selects the branch revision and loads its root", async () => {
  const calls = [];
  const { get } = setup(async url => {
    calls.push(url);
    if (url.endsWith("/repositories")) return { repositories: [{ name: "game", url: "lores://host/game" }] };
    if (url.endsWith("/branches")) return [{ name: "main", revision: "c".repeat(64) }];
    return [];
  });
  await get("loadRepositoryTreePage()");
  assert.equal(calls.length, 3);
  assert.equal(new URL(calls[2], "http://test").searchParams.get("revision"), "c".repeat(64));
  assert.equal(get("repositoryTree.loading"), false);
});
