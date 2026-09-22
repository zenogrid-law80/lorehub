import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";

const source = await readFile(new URL("../web/app.js", import.meta.url), "utf8");
const section = (start, end) => source.slice(source.indexOf(start), source.indexOf(end, source.indexOf(start)));
function setup(api = async () => []) {
  const node = () => ({ children: [], scrollTop: 0, scrollHeight: 1000, replaceChildren(...children) { this.children = children; }, append(child) { this.children.push(child); }, setAttribute(key, value) { this[key] = value; }, showModal() { this.open = true; } });
  const elements = Object.fromEntries(["pipeline-log", "detail-log-more", "detail-log-follow", "detail-log-restart", "detail-log-note", "detail-retry", "detail-repository", "detail-title", "detail-summary", "execution-graph", "execution-graph-section", "job-list", "job-count", "cancel-pipeline-button", "pipeline-detail-dialog", "detail-copy-link", "detail-permalink"].map(id => [id, node()]));
  const events = [];
  const state = { locale: "en", selectedId: "run", detailLogView: null };
  const context = vm.createContext({ state, elements, executionDetailRequest: 0, api, t: key => key,
    textNode: (textContent, className) => ({ textContent, className }), document: { createElement: node },
    repositoryName: value => value, revisionLabel: () => "revision", pipelineCategory: () => "test", toast() {},
    pipelinePermalink: id => `https://fixture/#pipelines?run=${id}`, recordPipelineLocation() {},
    renderDetailSummary() { events.push("summary"); }, renderExecutionGraph() { events.push("graph"); }, renderJobs() { events.push("jobs"); },
  });
  vm.runInContext(section("const detailLogCopy =", "function renderExecutionGraph("), context);
  vm.runInContext(section("function renderLogs(", "async function cancelPipeline("), context);
  const get = code => vm.runInContext(code, context);
  get("resetDetailLogs('run')");
  return { get, state, elements, events, context };
}
const detail = prunedAt => ({ pipeline: { id: "run", repository_url: "repo", status: "running", logs_pruned_at: prunedAt }, jobs: [] });
const rows = (after, count, content = "line\n") => Array.from({ length: count }, (_, i) => ({ id: after + i + 1, stream: "stdout", content }));

test("pruned logs show a retention notice with any remaining output", () => {
  const { get, elements } = setup();
  get("renderLogs([], null)");
  assert.equal(elements["pipeline-log"].children[0].textContent, "dynamic.noLogs");
  get("renderLogs([{stream:'stderr', content:'<script>literal</script>'}], '2026-09-01')");
  assert.deepEqual(elements["pipeline-log"].children.map(node => node.textContent), ["dynamic.logsPruned", "<script>literal</script>"]);
});

test("detail is visible before logs finish and full pages never drain automatically", async () => {
  let finish;
  const requests = [];
  const { get, events, state } = setup(async path => {
    requests.push(path);
    return path.includes("/logs?") ? new Promise(resolve => { finish = resolve; }) : detail(null);
  });
  const pending = get("loadPipelineDetail('run')");
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(events, ["summary", "graph", "jobs"]);
  assert.equal(requests.length, 2);
  finish(rows(0, 500)); await pending;
  await get("loadPipelineDetail('run')");
  assert.equal(requests.filter(path => path.includes("/logs?")).length, 1);
  assert.equal(state.detailLogView.more, true);
  assert.equal(state.detailLogView.after, 500);
});

test("manual paging is single-flight, cursor-based, and bounded by record count", async () => {
  const requests = [];
  const { get, state } = setup(async path => {
    requests.push(path);
    const params = new URL(path, "http://test").searchParams;
    assert.equal(params.get("limit"), "500");
    return rows(Number(params.get("after")), 500);
  });
  await Promise.all([get("loadPipelineLogs('run')"), get("loadPipelineLogs('run')")]);
  assert.equal(requests.length, 1);
  for (let i = 0; i < 4; i++) await get("loadPipelineLogs('run')");
  assert.equal(state.detailLogView.rows.length, 2000);
  assert.equal(state.detailLogView.rows[0].id, 501);
  assert.equal(state.detailLogView.after, 2500);
  assert.equal(state.detailLogView.trimmed, true);
});

test("buffer also bounds large records while keeping an independent cursor", async () => {
  const { get, state } = setup(async () => rows(0, 1, "x".repeat(700000)));
  await get("loadPipelineLogs('run')");
  assert.equal(state.detailLogView.rows[0].content.length, 512000);
  assert.equal(state.detailLogView.after, 1);
  assert.equal(state.detailLogView.trimmed, true);
});

test("paused logs retain reading position and skip automatic fetches", async () => {
  const requests = [];
  const { get, state, elements } = setup(async path => { requests.push(path); return detail(null); });
  Object.assign(state.detailLogView, { follow: false, started: true, more: false, rows: rows(0, 3) });
  elements["pipeline-log"].scrollTop = 123;
  await get("loadPipelineDetail('run')");
  assert.equal(requests.length, 1);
  assert.equal(elements["pipeline-log"].scrollTop, 123);
  assert.equal(elements["detail-log-follow"]["aria-pressed"], "false");
});

test("pruning invalidates pending pages and restarts from the remaining logs", async () => {
  let finishOld;
  const requests = [];
  const { get, state, elements } = setup(async path => {
    requests.push(path);
    if (!path.includes("/logs?")) return detail("2026-09-01");
    if (requests.length === 1) return new Promise(resolve => { finishOld = resolve; });
    return rows(10, 1, "retained");
  });
  const old = get("loadPipelineLogs('run')");
  await get("loadPipelineDetail('run')");
  finishOld(rows(0, 500, "deleted")); await old;
  assert.equal(state.detailLogView.after, 11);
  assert.equal(state.detailLogView.rows[0].content, "retained");
  assert.ok(requests[2].includes("after=0"));
  assert.equal(elements["pipeline-log"].children[0].textContent, "dynamic.logsPruned");
});

test("closed or replaced detail cannot receive late log output", async () => {
  let finish;
  const { get, state, elements } = setup(async () => new Promise(resolve => { finish = resolve; }));
  const pending = get("loadPipelineLogs('run')");
  state.selectedId = "other";
  get("resetDetailLogs('other'); renderLogs([], null)");
  finish(rows(0, 500, "old run")); await pending;
  assert.equal(state.detailLogView.id, "other");
  assert.equal(state.detailLogView.rows.length, 0);
  assert.ok(!elements["pipeline-log"].children.some(node => node.textContent === "old run"));
});

test("log failures retry the same cursor and access failure clears the entire detail", async () => {
  let failure = true;
  const requests = [];
  const { get, state, elements, context } = setup(async path => {
    requests.push(path);
    if (failure) throw new Error("temporary failure");
    return rows(10, 1);
  });
  state.detailLogView.after = 10;
  await get("loadPipelineLogs('run')");
  assert.equal(state.detailLogView.after, 10);
  assert.equal(elements["detail-log-more"].textContent, "Retry logs");
  failure = false;
  await get("loadPipelineLogs('run')");
  assert.equal(requests[0], requests[1]);
  context.api = async () => { throw Object.assign(new Error("not found"), { status: 404 }); };
  await get("loadPipelineLogs('run')");
  assert.equal(state.detailLogView, null);
  assert.equal(elements["cancel-pipeline-button"].hidden, true);
  assert.equal(elements["execution-graph-section"].hidden, true);
  assert.equal(elements["detail-retry"].hidden, false);
});

test("failed detail refresh removes stale content before offering retry", async () => {
  const { get, state, elements } = setup(async () => { throw new Error("unavailable"); });
  state.detailLogView.rows = rows(0, 1, "old content");
  await assert.rejects(get("loadPipelineDetail('run')"), /unavailable/);
  assert.equal(state.detailLogView, null);
  assert.equal(elements["detail-summary"].children.length, 0);
  assert.equal(elements["detail-retry"].hidden, false);
});

test("restart fetches the first page and keeps its beginning visible", async () => {
  const requests = [];
  const { get, state, elements } = setup(async path => { requests.push(path); return rows(0, 500); });
  state.detailLogView.after = 2500;
  elements["pipeline-log"].scrollTop = 5000;
  await get("restartDetailLogs()");
  assert.ok(requests[0].includes("after=0"));
  assert.equal(state.detailLogView.rows[0].id, 1);
  assert.equal(state.detailLogView.follow, false);
  assert.equal(elements["pipeline-log"].scrollTop, 0);
});
