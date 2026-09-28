import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";
const source = await readFile(new URL("../web/operations.js", import.meta.url), "utf8");
function setup(api) {
  const context = vm.createContext({ state: { section: "operations", locale: "en", user: { id: "admin", role: "admin" } }, api, AbortController, document: { getElementById: () => null }, toast() {}, window: { setTimeout, clearTimeout }, renders: 0 });
  vm.runInContext(source, context);
  vm.runInContext("renderOperations = () => { renders++; }", context);
  return { context, get: code => vm.runInContext(code, context) };
}

test("diagnostics preserves unknown values and uses binary storage units", () => {
  const { get } = setup(async () => ({}));
  assert.equal(get("operationsAge(null)"), "—");
  assert.equal(get("operationsAge(125.8)"), "2m 5s");
  assert.equal(get("operationsAge(-1)"), "0m 0s");
  assert.equal(get("operationsBytes(1073741824)"), "1.0 GiB");
  assert.equal(get("ot('stale')"), "Check overdue (>2 min)");
  get("state.locale='ko'");
  assert.equal(get("ot('unknown')"), "아직 관측되지 않음");
});

test("failed refresh removes the old snapshot and successful retry clears the error", async () => {
  let fail = true;
  const { get } = setup(async () => { if (fail) throw new Error("forbidden"); return { observed_at: "now" }; });
  get("operations.snapshot = { observed_at: 'old' }");
  await get("loadOperations()");
  assert.equal(get("operations.snapshot"), null);
  assert.equal(get("operations.error"), "forbidden");
  assert.equal(get("operations.loading"), false);
  fail = false;
  await get("loadOperations()");
  assert.equal(get("operations.snapshot.observed_at"), "now");
  assert.equal(get("operations.error"), null);
});

test("overlapping polls are skipped and an invalidated response cannot restore administrator data", async () => {
  let resolve, calls = 0;
  const { get } = setup(() => { calls++; return new Promise(done => { resolve = done; }); });
  const pending = get("loadOperations()");
  const initialRenders = get("renders");
  await get("loadOperations()");
  assert.equal(calls, 1);
  get("resetOperationsState()");
  resolve({ secret: "stale response" });
  await pending;
  assert.equal(get("operations.snapshot"), null);
  assert.equal(get("renders"), initialRenders);
  assert.equal(get("operations.loading"), false);
});

test("reset aborts the old request without letting its completion unlock a new request", async () => {
  const requests = [];
  const { get } = setup((path, { signal }) => new Promise(resolve => requests.push({ signal, resolve })));
  const old = get("loadOperations()");
  get("resetOperationsState()");
  assert.equal(requests[0].signal.aborted, true);
  const current = get("loadOperations()");
  requests[0].resolve({ observed_at: "old" });
  await old;
  assert.equal(get("operations.loading"), true);
  assert.equal(get("operations.snapshot"), null);
  await get("loadOperations()");
  assert.equal(requests.length, 2);
  requests[1].resolve({ observed_at: "current" });
  await current;
  assert.equal(get("operations.loading"), false);
  assert.equal(get("operations.snapshot.observed_at"), "current");
});

test("execution detail keeps polling from operations while summary refresh remains throttled", async () => {
  const app = await readFile(new URL("../web/app.js", import.meta.url), "utf8");
  let summaries = 0, now = Date.now();
  const details = [];
  const context = vm.createContext({
    Date: { now: () => now },
    document: { hidden: false }, state: { section: "operations", selectedId: "active-run", user: { id: "admin", role: "admin" } },
    operations: { lastAttempt: Date.now() },
    elements: { "pipeline-detail-dialog": { open: true } },
    loadOperations: async () => { summaries++; },
    loadPipelineDetail: async id => { details.push(id); },
  });
  vm.runInContext(app.slice(app.indexOf("async function refreshActiveViews()"), app.indexOf("function csrfToken()")), context);
  await vm.runInContext("refreshActiveViews()", context);
  assert.equal(summaries, 0);
  assert.deepEqual(details, ["active-run"]);
  now += 5000;
  vm.runInContext("operations.lastAttempt = 0", context);
  await vm.runInContext("refreshActiveViews()", context);
  assert.equal(summaries, 1);
  assert.deepEqual(details, ["active-run", "active-run"]);
});
