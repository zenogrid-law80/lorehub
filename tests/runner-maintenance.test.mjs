import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";
const source = await readFile(new URL("../web/app.js", import.meta.url), "utf8");
const code = source.slice(source.indexOf("let runnerListRequest ="), source.indexOf("async function loadPipelineGraphs("))
  + source.slice(source.indexOf("function runnerMaintenanceLabel("), source.indexOf("function renderRunners("))
  + source.slice(source.indexOf("async function setRunnerDraining("), source.indexOf("async function removeRunner("));
function setup(api) {
  const state = { user: { role: "admin" }, runners: [{ id: "one", draining: false, busy: true }], updatedAt: {} };
  const notices = [];
  const context = vm.createContext({ state, api, notices, elements: { "refresh-button": {} },
    renderRunners() {}, renderUpdatedLabels() {}, t: key => key, csrfToken: () => "fixture-csrf",
    toast: (message, kind) => notices.push({ message, kind }),
  });
  vm.runInContext(code, context);
  return { state, notices, get: expression => vm.runInContext(expression, context) };
}

test("a poll from before pausing cannot overwrite the confirmed maintenance state", async () => {
  let finishOld, reads = 0;
  const { get, state } = setup(async (_, options) => {
    if (options?.method === "POST") return;
    if (++reads === 1) return new Promise(resolve => { finishOld = resolve; });
    return [{ id: "one", draining: true, busy: true }];
  });
  const old = get("loadRunners(false)");
  await get("setRunnerDraining('one', true)");
  finishOld([{ id: "one", draining: false, busy: true }]);
  await old;
  assert.equal(state.runners[0].draining, true);
  assert.equal(get("elements['refresh-button'].disabled"), false);
});

test("pending maintenance rejects duplicate clicks and polling until the write finishes", async () => {
  let finish;
  const writes = [];
  const { get } = setup(async (path, options) => {
    if (options?.method === "POST") {
      writes.push({ path, options });
      return new Promise(resolve => { finish = resolve; });
    }
    return [{ id: "one", draining: true, busy: true }];
  });
  const pending = get("setRunnerDraining('one', true)");
  assert.equal(get("runnerMutations.has('one')"), true);
  await get("setRunnerDraining('one', true)");
  await get("loadRunners(false)");
  assert.equal(writes.length, 1);
  assert.equal(writes[0].path, "/api/v1/runners/one/drain");
  assert.equal(writes[0].options.headers["X-CSRF-Token"], "fixture-csrf");
  assert.deepEqual(JSON.parse(writes[0].options.body), { draining: true });
  finish(); await pending;
  assert.equal(get("runnerMutations.size"), 0);
});

test("failed writes keep observed state, show an error and leave controls usable", async () => {
  const { get, state, notices } = setup(async (_, options) => {
    if (options?.method === "POST") throw new Error("forbidden");
    return [{ id: "one", draining: false, busy: false }];
  });
  await get("setRunnerDraining('one', true)");
  assert.equal(state.runners[0].draining, false);
  assert.deepEqual(notices, [{ message: "forbidden", kind: "error" }]);
  assert.equal(get("runnerMutations.size"), 0);
});

test("read-only users cannot mutate and restricted work is still shown as draining", async () => {
  const { get, state } = setup(async () => { assert.fail("non-admin must not send mutations"); });
  state.user.role = "user";
  await get("setRunnerDraining('one', true)");
  assert.equal(get("runnerMaintenanceLabel({ draining:true, busy:true, current_pipeline_id:null })"), "runner.draining");
  assert.equal(get("runnerMaintenanceLabel({ draining:true, busy:false })"), "runner.drained");
});
