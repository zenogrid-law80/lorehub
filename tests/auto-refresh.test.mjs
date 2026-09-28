import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";

const source = await readFile(new URL("../web/app.js", import.meta.url), "utf8");
const polling = source.slice(source.indexOf("function stopAutoRefresh()"), source.indexOf("function csrfToken()"));
function setup() {
  let now = 100000;
  const calls = [];
  const state = { section: "pipelines", selectedId: null, repositoryScope: "repo", repositoryBranch: "main", pipelines: [], pipelineGraphs: [], runners: [], repositoryLinkOperations: [] };
  const document = { hidden: false };
  const elements = { "pipeline-detail-dialog": { open: true } };
  const load = name => async () => { calls.push([name, now]); return true; };
  const context = vm.createContext({ state, document, elements, Date: { now: () => now },
    navigator: { onLine: true }, window: { clearInterval() {} },
    pipelineHistoryParameters: () => new URLSearchParams(), isManagement: () => false,
    loadPipelines: load("pipelines"), loadPipelineGraphs: load("graphs"), loadRunners: load("runners"), loadPipelineDetail: load("detail"),
    operations: { lastAttempt: 0 }, workspaceOverview: { lastAttempt: 0 },
    loadOperations: load("operations"), loadOverview: load("overview"), loadRepositoryLinkOperations: load("links"),
  });
  vm.runInContext(polling, context);
  return { state, document, elements, calls, context, advance: ms => { now += ms; }, get: code => vm.runInContext(code, context) };
}

test("idle lists poll every 30 seconds and active lists every 5 seconds", async () => {
  const { get, calls, advance, state } = setup();
  await get("refreshActiveViews()");
  for (let i = 0; i < 5; i++) { advance(5000); await get("refreshActiveViews()"); }
  assert.equal(calls.length, 1);
  advance(5000); await get("refreshActiveViews()");
  assert.equal(calls.length, 2);
  state.pipelines = [{ status: "running" }];
  advance(5000); await get("refreshActiveViews()");
  assert.equal(calls.length, 3);
});

test("reconnecting checks due work without overlapping requests or bypassing backoff", async () => {
  const { get, context, state, advance } = setup();
  Object.assign(state, { user: { id: "user", role: "user" }, refreshTimer: 1 });
  let finish, calls = 0;
  context.loadPipelines = () => { calls++; return new Promise(resolve => { finish = resolve; }); };
  context.navigator.onLine = false;
  get("refreshWhenAvailable()");
  assert.equal(calls, 0);
  context.navigator.onLine = true;
  get("refreshWhenAvailable(); refreshWhenAvailable()");
  assert.equal(calls, 1);
  finish(false);
  await new Promise(resolve => setImmediate(resolve));
  advance(5000); get("refreshWhenAvailable()");
  assert.equal(calls, 1);
  advance(25000); get("refreshWhenAvailable()");
  assert.equal(calls, 2);
  finish(true);
  await new Promise(resolve => setImmediate(resolve));
  get("stopAutoRefresh()");
  advance(60000); get("refreshWhenAvailable()");
  assert.equal(calls, 2);
});

test("finished details stop polling while a running detail stays fast over an idle list", async () => {
  const { get, calls, advance, state } = setup();
  state.selectedId = "run";
  state.detailLogView = { id: "run", status: "running" };
  await get("refreshActiveViews()");
  advance(5000); await get("refreshActiveViews()");
  assert.equal(calls.filter(([name]) => name === "detail").length, 2);
  assert.equal(calls.filter(([name]) => name === "pipelines").length, 1);
  state.detailLogView.status = "succeeded";
  advance(30000); await get("refreshActiveViews()");
  assert.equal(calls.filter(([name]) => name === "detail").length, 2);
  state.selectedId = "other";
  await get("refreshActiveViews()");
  assert.equal(calls.filter(([name]) => name === "detail").length, 3);
});

test("failed endpoints back off to 60 seconds and recover without delaying other views", async () => {
  const { get, calls, advance, state, context } = setup();
  state.selectedId = "run";
  state.pipelines = [{ status: "running" }];
  let attempts = 0, fail = true;
  context.loadPipelineDetail = async () => { attempts++; if (fail) throw new Error("offline"); };
  await get("refreshActiveViews()");
  for (const wait of [10000, 20000, 40000, 60000, 60000]) {
    advance(wait - 5000); await get("refreshActiveViews()");
    const before = attempts;
    advance(5000); await get("refreshActiveViews()");
    assert.equal(attempts, before + 1);
  }
  assert.ok(calls.length > attempts);
  fail = false;
  advance(60000); await get("refreshActiveViews()");
  const recovered = attempts;
  advance(5000); await get("refreshActiveViews()");
  assert.equal(attempts, recovered + 1);
});

test("handled loader failures also back off", async () => {
  const { get, advance, state, context } = setup();
  state.pipelines = [{ status: "running" }];
  let attempts = 0;
  context.loadPipelines = async () => { attempts++; return false; };
  await get("refreshActiveViews()");
  advance(5000); await get("refreshActiveViews()");
  assert.equal(attempts, 1);
  advance(5000); await get("refreshActiveViews()");
  assert.equal(attempts, 2);
});

test("slow requests never overlap and old completions cannot overwrite a new scope's timer", async () => {
  const { get, advance, state, context } = setup();
  let finish, attempts = 0;
  context.loadPipelines = async () => { attempts++; if (attempts === 1) return new Promise(resolve => { finish = resolve; }); return true; };
  const pending = get("refreshActiveViews()");
  advance(60000); await get("refreshActiveViews()");
  assert.equal(attempts, 1);
  state.repositoryScope = "other";
  await get("refreshActiveViews()");
  assert.equal(attempts, 2);
  finish(false); await pending;
  advance(30000); await get("refreshActiveViews()");
  assert.equal(attempts, 3);
});

test("hidden tabs and older history pages skip polling while visible details remain independent", async () => {
  const { get, calls, document, state } = setup();
  document.hidden = true;
  await get("refreshActiveViews()");
  assert.equal(calls.length, 0);
  document.hidden = false; state.pipelineHasOlderPages = true;
  await get("refreshActiveViews()");
  assert.equal(calls.length, 0);
  state.selectedId = "run";
  await get("refreshActiveViews()");
  assert.deepEqual(calls.map(([name]) => name), ["detail"]);
});

test("runner activity uses busy status even when the pipeline ID is restricted", async () => {
  const { get, calls, advance, state } = setup();
  state.section = "runners"; state.runners = [{ busy: true, current_pipeline_id: null }];
  await get("refreshActiveViews()");
  advance(5000); await get("refreshActiveViews()");
  assert.equal(calls.length, 2);
  state.runners[0].busy = false;
  advance(5000); await get("refreshActiveViews()");
  assert.equal(calls.length, 2);
});

test("admin alerts refresh across pages without interrupting drafts, while hidden or forbidden views stay quiet", async () => {
  const { get, calls, advance, state, document, context } = setup();
  state.user = { id: "admin", role: "admin" }; state.section = "ci-settings";
  await get("refreshActiveViews()");
  assert.deepEqual(calls.map(([name]) => name), ["operations"]);
  advance(30000); document.hidden = true;
  await get("refreshActiveViews()");
  assert.equal(calls.length, 1);
  document.hidden = false; state.section = "accounts"; context.isManagement = () => true;
  await get("refreshActiveViews()");
  assert.equal(calls.length, 2);
  advance(30000); context.operations.forbidden = true;
  await get("refreshActiveViews()");
  assert.equal(calls.length, 2);
  context.operations.forbidden = false; state.user.role = "user";
  await get("refreshActiveViews()");
  assert.equal(calls.length, 2);
});

test("session expiry during initialization cannot restart polling or continue navigation", async () => {
  const { get, context } = setup();
  let timers = 0, navigation = 0, reloads = 0;
  Object.assign(context, {
    fetch: async path => path === "/api/v1/me"
      ? { ok: true, json: async () => ({ id: "user", role: "user" }) }
      : { status: 401 },
    restorePipelineLogin() {}, renderUser() {}, showLogin() {}, toast() {}, t: key => key,
    loadRepositories: async () => { try { await get("api('/api/v1/repositories')"); } catch (_) {} },
    loadRunners: async () => {}, syncPipelineLocation: async () => { navigation++; },
  });
  Object.assign(context.elements, { "loading-view": {}, "app-view": {} });
  Object.assign(context.window, { location: { reload() { reloads++; } }, setInterval() { timers++; return 1; } });
  vm.runInContext(source.slice(source.indexOf("async function initialize()"), source.indexOf("function sectionFromHash()")), context);
  vm.runInContext(source.slice(source.indexOf("async function api("), source.indexOf("async function loadPipelines(")), context);
  await get("initialize()");
  assert.equal(reloads, 1);
  assert.equal(navigation, 0);
  assert.equal(timers, 0);
});

test("returning to a scope with an unfinished poll reuses its request and failure backoff", async () => {
  const { get, context, state, advance } = setup();
  state.pipelines = [{ status: "running" }];
  const requests = [];
  context.loadPipelines = () => new Promise(resolve => {
    requests.push({ scope: state.repositoryScope, resolve });
  });
  const first = get("refreshActiveViews()");
  state.repositoryScope = "other";
  const second = get("refreshActiveViews()");
  state.repositoryScope = "repo";
  const returned = get("refreshActiveViews()");
  assert.deepEqual(requests.map(request => request.scope), ["repo", "other"]);
  await returned;
  requests[1].resolve(true); await second;
  advance(5000);
  await get("refreshActiveViews()");
  assert.equal(requests.length, 2);
  requests[0].resolve(false); await first;
  advance(5000);
  await get("refreshActiveViews()");
  assert.equal(requests.length, 2);
  advance(5000);
  const retry = get("refreshActiveViews()");
  assert.equal(requests.length, 3);
  requests[2].resolve(true); await retry;
});
