import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";
const source = await readFile(new URL("../web/repository-context.js", import.meta.url), "utf8");
const app = await readFile(new URL("../web/app.js", import.meta.url), "utf8");
const fn = (name, next) => app.slice(app.indexOf(name), app.indexOf(next, app.indexOf(name)));
function setup(extra = {}) {
  const context = vm.createContext({ URLSearchParams, state: { repositoryScope: "", repositories: [] }, ...extra });
  vm.runInContext(source, context);
  return { context, get: code => vm.runInContext(code, context) };
}

test("repository routes round-trip complete URLs and keep global pages unscoped", () => {
  const { context, get } = setup();
  const url = "lores://host:41337/demo?value=a&b=한글#revision";
  context.scope = url;
  assert.equal(get("repositoryRoute(repositorySectionHash('pipelines', scope)).repository"), url);
  assert.equal(get("repositoryRoute(repositorySectionHash('graphs', scope)).section"), "graphs");
  assert.equal(get("repositorySectionHash('runners', scope)"), "#runners");
  assert.equal(get("repositoryRoute('#operations?repository=anything').repository"), "");
  assert.equal(get("repositoryRoute('#pipelines').repository"), "");
});

test("repository configuration candidates match the full URL and never fall back to another repository", () => {
  const { get } = setup({ state: { repositoryScope: "lores://one/demo", repositories: [{ name: "demo", url: "lores://one/demo" }, { name: "demo", url: "lores://two/demo" }] } });
  assert.equal(get("scopedRepositories().length"), 1);
  assert.equal(get("scopedRepositories()[0].url"), "lores://one/demo");
  get("state.repositoryScope='lores://missing/demo'");
  assert.equal(get("scopedRepositories().length"), 0);
});

function historySetup(api) {
  const { context, get } = setup({
    state: { repositoryScope: "lores://host/one", pipelines: [], pipelineRequest: 0, pipelineLoading: false, pipelineNextBefore: null, updatedAt: {} },
    api, elements: { "refresh-button": {}, "load-more-pipelines": {} },
    renderStats() {}, renderPipelines() {}, renderUpdatedLabels() {}, toast() {}, t: x => x,
  });
  vm.runInContext(fn("async function loadPipelines(", "async function loadRepositories("), context);
  return { context, get };
}

test("history pagination includes repository scope and preserves loaded older pages", async () => {
  const requests = [];
  const { context, get } = historySetup(async url => {
    requests.push(new URL(url, "http://test"));
    return requests.length === 1 ? { pipelines: [{ id: "new" }], next_before: "new" } : { pipelines: [{ id: "old" }], next_before: null };
  });
  await get("loadPipelines(false)");
  await get("loadPipelines(false, true)");
  assert.deepEqual(Array.from(get("state.pipelines"), p => p.id), ["new", "old"]);
  for (const request of requests) assert.equal(request.searchParams.get("repository_url"), "lores://host/one");
  assert.equal(requests[1].searchParams.get("before"), "new");
  assert.equal(get("state.pipelineHasOlderPages"), true);
  let polls = 0;
  Object.assign(get("state"), { section: "pipelines", selectedId: null });
  Object.assign(context, { document: { hidden: false }, isManagement: () => false, loadPipelines: async () => { polls++; } });
  vm.runInContext(fn("async function refreshActiveViews()", "function csrfToken()"), context);
  await get("refreshActiveViews()");
  assert.equal(polls, 0);
});

test("a late history response from the previous repository cannot replace the selected repository", async () => {
  let finishOld;
  const { get } = historySetup(async url => {
    if (url.includes("%2Fone")) return new Promise(resolve => { finishOld = resolve; });
    return { pipelines: [{ id: "two-run" }], next_before: null };
  });
  const old = get("loadPipelines(false)");
  get("state.repositoryScope='lores://host/two'; state.pipelineRequest++; state.pipelineLoading=false;");
  await get("loadPipelines(false)");
  finishOld({ pipelines: [{ id: "one-run" }], next_before: "old-cursor" });
  await old;
  assert.deepEqual(Array.from(get("state.pipelines"), p => p.id), ["two-run"]);
  assert.equal(get("state.pipelineNextBefore"), null);
  assert.equal(get("state.pipelineLoading"), false);
});

test("declining to discard CI edits restores the repository URL and keeps the draft", async () => {
  const replacements = [];
  const { context, get } = setup({
    state: { section: "ci-settings", repositoryScope: "lores://host/one", repositoryConfigEditing: true, repositoryConfigDraft: { unchanged: true } },
    window: { location: { hash: "#ci-settings?repository=lores%3A%2F%2Fhost%2Ftwo" } },
    availableSections: () => ["ci-settings"], discardRepositoryConfigEdit: () => false,
    history: { replaceState: (_, __, url) => replacements.push(url) },
    document: { getElementById: () => ({}) },
  });
  vm.runInContext(fn("async function showSection(", "function updateSectionSearch("), context);
  await get("showSection('ci-settings')");
  assert.equal(get("state.repositoryScope"), "lores://host/one");
  assert.equal(get("state.repositoryConfigDraft.unchanged"), true);
  assert.equal(replacements[0], "#ci-settings?repository=lores%3A%2F%2Fhost%2Fone");
});

test("history URL restores exact filters and sends literal search through every page", async () => {
  const requests = [];
  const { context, get } = historySetup(async url => {
    requests.push(new URL(url, "http://test"));
    return { pipelines: [], next_before: requests.length === 1 ? "cursor" : null };
  });
  context.document = { querySelectorAll: () => [] };
  context.elements["pipeline-search"] = {};
  context.hash = "#pipelines?branch=release%2F2026&pipeline_name=Build+%26+Test&status=active&q=100%25_%ED%95%9C%EA%B8%80";
  get("state.section='pipelines'; restorePipelineHistoryFilters(hash)");
  assert.equal(get("state.query"), "100%_한글");
  assert.equal(get("state.filter"), "running");
  await get("loadPipelines(false)");
  await get("loadPipelines(false, true)");
  for (const request of requests) {
    assert.equal(request.searchParams.get("branch"), "release/2026");
    assert.equal(request.searchParams.get("pipeline_name"), "Build & Test");
    assert.equal(request.searchParams.get("q"), "100%_한글");
    assert.equal(request.searchParams.get("status"), "active");
  }
  assert.equal(get("state.pipelineNameFilter"), "Build & Test");
  get("state.section='graphs'; restorePipelineHistoryFilters(hash)");
  assert.equal(get("pipelineHistoryParameters().has('q')"), false);
  assert.equal(get("state.pipelineBranchFilter"), "");
});

test("typing resets pagination immediately and debounces requests without letting polling bypass it", async () => {
  const callbacks = new Map();
  let serial = 0, loads = 0;
  const urls = [];
  const { context, get } = historySetup(async () => { loads++; return { pipelines: [], next_before: null }; });
  context.window = { clearTimeout: id => callbacks.delete(id), setTimeout: fn => { const id = ++serial; callbacks.set(id, fn); return id; } };
  context.history = { replaceState: (_, __, url) => urls.push(url) };
  context.document = { hidden: false };
  context.isManagement = () => false;
  vm.runInContext(fn("async function refreshActiveViews()", "function csrfToken()"), context);
  get("Object.assign(state, {section:'pipelines', query:'first', filter:'failed', pipelineNextBefore:'old', pipelineHasOlderPages:true}); searchPipelineHistory(250)");
  assert.equal(get("state.pipelineNextBefore"), null);
  assert.equal(get("state.pipelineHasOlderPages"), false);
  get("state.query='second'; searchPipelineHistory(250)");
  assert.equal(callbacks.size, 1);
  await get("refreshActiveViews()");
  assert.equal(loads, 0);
  assert.ok(urls.at(-1).includes("q=second"));
  assert.ok(urls.at(-1).includes("status=failed"));
  assert.ok(urls.at(-1).includes("repository=lores%3A%2F%2Fhost%2Fone"));
  get("invalidatePipelineHistory()");
  assert.equal(callbacks.size, 0);
});

test("a slow previous search cannot overwrite results or cursors for the new search", async () => {
  let resolveOld;
  const { context, get } = historySetup(async url => {
    if (new URL(url, "http://test").searchParams.get("q") === "old") return new Promise(resolve => { resolveOld = resolve; });
    return { pipelines: [{ id: "new-match" }], next_before: null };
  });
  context.window = { clearTimeout() {} };
  get("state.section='pipelines'; state.query='old'");
  const pending = get("loadPipelines(false)");
  get("state.query='new'; invalidatePipelineHistory()");
  await get("loadPipelines(false)");
  resolveOld({ pipelines: [{ id: "old-match" }], next_before: "obsolete" });
  await pending;
  assert.deepEqual(Array.from(get("state.pipelines"), p => p.id), ["new-match"]);
  assert.equal(get("state.pipelineNextBefore"), null);
});
