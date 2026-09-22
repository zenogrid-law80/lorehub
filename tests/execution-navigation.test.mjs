import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";
const source = await readFile(new URL("../web/repository-context.js", import.meta.url), "utf8");

function setup(hash = "#pipelines?branch=release&q=build") {
  const location = new URL(`https://hub.example/app?private=excluded${hash}`);
  const entries = [{ url: location.href, data: null }];
  let index = 0, backCalls = 0;
  const history = {
    get state() { return entries[index].data; },
    pushState(data, _, path) { location.href = new URL(path, location).href; entries.splice(index + 1); entries.push({ url: location.href, data }); index++; },
    replaceState(data, _, path) { location.href = new URL(path, location).href; entries[index] = { url: location.href, data }; },
    back() { backCalls++; if (index) location.href = entries[--index].url; },
    forward() { if (index + 1 < entries.length) location.href = entries[++index].url; },
  };
  const state = { user: { role: "user" }, selectedId: null, pipelines: ["loaded-older-run"], pipelineHasOlderPages: true };
  const dialog = { open: false, close() { this.open = false; } };
  const opened = [], pages = [];
  const context = vm.createContext({ URL, URLSearchParams, window: { location }, history, state, elements: { "pipeline-detail-dialog": dialog }, executionDetailRequest: 0,
    sectionFromHash: () => location.hash.split("?")[0].slice(1),
    showSection: async section => { pages.push(section); get("rememberPipelinePage(window.location.hash)"); },
    openPipeline: async (id, options) => { assert.equal(options.fromLocation, true); state.selectedId = id; dialog.open = true; opened.push(id); },
  });
  vm.runInContext(source, context);
  const get = code => vm.runInContext(code, context);
  return { get, state, dialog, location, history, entries, pages, opened, context, backCalls: () => backCalls };
}

test("opening related runs uses one detail entry and retains original search conditions", () => {
  const { get, location, entries } = setup();
  get("recordPipelineLocation('run-one')");
  assert.equal(entries.length, 2);
  assert.equal(location.hash, "#pipelines?branch=release&q=build&run=run-one");
  get("recordPipelineLocation('run-two')");
  get("recordPipelineLocation('run-two')");
  assert.equal(entries.length, 2);
  assert.equal(location.hash, "#pipelines?branch=release&q=build&run=run-two");
});

test("Back and Forward toggle detail without reloading or losing older history pages", async () => {
  const { get, state, dialog, history, opened, pages } = setup();
  get("rememberPipelinePage(window.location.hash); recordPipelineLocation('run-one')");
  await get("syncPipelineLocation()");
  assert.equal(dialog.open, true);
  history.back(); await get("syncPipelineLocation()");
  assert.equal(dialog.open, false);
  assert.equal(state.selectedId, null);
  assert.deepEqual(state.pipelines, ["loaded-older-run"]);
  assert.equal(state.pipelineHasOlderPages, true);
  history.forward(); await get("syncPipelineLocation()");
  assert.deepEqual(opened, ["run-one", "run-one"]);
  assert.deepEqual(pages, []);
});

test("closing an in-app detail returns to its parent, but a direct link closes locally", async () => {
  const inside = setup();
  inside.get("recordPipelineLocation('run-one'); closePipelineLocation('run-one')");
  assert.equal(inside.backCalls(), 1);
  assert.equal(inside.location.hash, "#pipelines?branch=release&q=build");
  const direct = setup("#pipelines?run=run-one");
  await direct.get("syncPipelineLocation(true)");
  assert.deepEqual(direct.opened, ["run-one"]);
  direct.get("closePipelineLocation('run-one')");
  assert.equal(direct.backCalls(), 0);
  assert.equal(direct.location.hash, "#pipelines");
});

test("a reloaded detail never uses a prior document's history ownership", () => {
  const { get, history, location, backCalls } = setup("#operations?run=run-one");
  history.replaceState({ lorehubDetail: { owner: "old-document", base: "#operations" } }, "", location.href);
  get("closePipelineLocation('run-one')");
  assert.equal(backCalls(), 0);
  assert.equal(location.hash, "#operations");
  assert.equal(history.state.lorehubDetail, undefined);
});

test("permalinks use the common history page without search terms or URL credentials", () => {
  const { get } = setup("#operations?repository=secret&q=private&run=run-one");
  assert.equal(get("pipelinePermalink('run-one')"), "https://hub.example/app#pipelines?run=run-one");
  assert.equal(get("pipelineLocation('#pipelines?run=%3Cscript%3E').id"), null);
});

test("later navigation wins while an earlier page is still loading", async () => {
  let finish;
  const { get, context, location, opened } = setup("#graphs?run=old-run");
  context.showSection = async () => new Promise(resolve => { finish = resolve; });
  const pending = get("syncPipelineLocation()");
  location.hash = "#pipelines?run=new-run";
  context.showSection = async () => get("rememberPipelinePage(window.location.hash)");
  await get("syncPipelineLocation()");
  finish(); await pending;
  assert.deepEqual(opened, ["new-run"]);
});

test("links wait for authentication and reopen once signed in", async () => {
  const { get, state, opened } = setup("#pipelines?run=run-one");
  state.user = null;
  await get("syncPipelineLocation(true)");
  assert.deepEqual(opened, []);
  state.user = { role: "user" };
  await get("syncPipelineLocation(true)");
  assert.deepEqual(opened, ["run-one"]);
});

test("login return restores a run once, expires it, and never overrides a new destination", () => {
  const { get, context, location } = setup("#pipelines?status=failed&run=run-one");
  const storage = new Map();
  context.window.sessionStorage = { getItem: key => storage.get(key), setItem: (key, value) => storage.set(key, value), removeItem: key => storage.delete(key) };
  get("rememberPipelineLogin()");
  location.hash = "";
  get("restorePipelineLogin()");
  assert.equal(location.hash, "#pipelines?status=failed&run=run-one");
  assert.equal(storage.size, 0);
  get("rememberPipelineLogin()");
  location.hash = "#pipelines?run=run-two";
  get("restorePipelineLogin()");
  assert.equal(location.hash, "#pipelines?run=run-two");
  location.hash = "";
  storage.set("lorehub_pending_run", JSON.stringify({ hash: "#pipelines?run=old-run", at: Date.now() - 700000 }));
  get("restorePipelineLogin()");
  assert.equal(location.hash, "");
  storage.set("lorehub_pending_run", JSON.stringify({ hash: "https://other.example/#pipelines?run=run-one", at: Date.now() }));
  get("restorePipelineLogin()");
  assert.equal(location.origin, "https://hub.example");
  assert.equal(location.hash, "");
});
