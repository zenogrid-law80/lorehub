import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";
const source = await readFile(new URL("../web/app.js", import.meta.url), "utf8");
const code = source.slice(source.indexOf("let runnerListRequest ="), source.indexOf("async function loadPipelineGraphs("))
  + source.slice(source.indexOf("let runnerDiagnosticId ="), source.indexOf("function renderRunners("))
  + source.slice(source.indexOf("function runnerAction("), source.indexOf("async function setRunnerDraining("));
function node() {
  return { textContent: "", children: [], open: false, replaceChildren() { this.children = []; },
    append(...children) { this.children.push(...children); }, showModal() { this.open = true; } };
}
function setup(api = async () => []) {
  const elements = Object.fromEntries(["dialog", "title", "status", "hint", "facts", "freshness"].map(key => [`runner-diagnostics-${key}`, node()]));
  elements["refresh-button"] = node();
  const state = { locale: "ko", user: { role: "user" }, updatedAt: {}, runners: [{
    id: "runner", name: "Build machine", os: "linux", arch: "x86_64", version: "0.2.21", diagnostic: "poll_stalled",
    docker_available: true, busy: false, last_claim_at: null, observed_at: "2026-09-22T01:00:00Z",
  }] };
  const context = vm.createContext({ elements, state, api, document: { createElement: node },
    t: key => key, localeTag: () => "ko-KR", dockerStatusLabel: () => "Installed",
    renderUpdatedLabels() {}, toast() {}, setRunnerDraining() { assert.fail("non-admin mutation"); },
  });
  vm.runInContext(code + "\nfunction renderRunners() { renderRunnerDiagnostics(); }", context);
  const get = expression => vm.runInContext(expression, context);
  const action = name => {
    context.event = { target: { closest: () => ({ disabled: false, dataset: { runnerAction: name, runnerId: "runner" } }) } };
    get("runnerAction(event)");
  };
  return { elements, state, get, action };
}

test("read-only users can open diagnostics without gaining maintenance controls", () => {
  const { elements, action } = setup();
  action("diagnostics");
  assert.equal(elements["runner-diagnostics-dialog"].open, true);
  assert.equal(elements["runner-diagnostics-status"].textContent, "작업 요청 지연");
  assert.match(elements["runner-diagnostics-hint"].textContent, /자동 업데이트/);
  action("drain"); // Must not invoke the mutation helper.
});

test("open diagnostics follow fresh polling and locale changes", async () => {
  const { elements, state, get, action } = setup(async () => [{ id: "runner", name: "Build machine", diagnostic: "busy", version: "0.2.21" }]);
  action("diagnostics");
  await get("loadRunners(false)");
  assert.equal(elements["runner-diagnostics-status"].textContent, "작업 실행 중");
  state.locale = "en";
  get("renderRunnerDiagnostics()");
  assert.equal(elements["runner-diagnostics-status"].textContent, "Running work");
  assert.equal(elements["runner-diagnostics-dialog"].open, true);
});

test("failed refresh marks cached diagnostics and recovery clears the warning", async () => {
  let fail = true;
  const { elements, get, action } = setup(async () => {
    if (fail) throw new Error("offline");
    return [{ id: "runner", name: "Build machine", diagnostic: "ready" }];
  });
  action("diagnostics");
  await get("loadRunners(false)");
  assert.match(elements["runner-diagnostics-freshness"].textContent, /갱신 실패/);
  assert.equal(elements["runner-diagnostics-status"].textContent, "작업 요청 지연");
  fail = false;
  await get("loadRunners(false)");
  assert.equal(elements["runner-diagnostics-freshness"].textContent, "");
  assert.equal(elements["runner-diagnostics-status"].textContent, "작업 요청 정상");
});

test("removed runners and older servers do not present invented current state", async () => {
  const { elements, state, get, action } = setup();
  state.runners[0].diagnostic = undefined;
  action("diagnostics");
  assert.equal(elements["runner-diagnostics-status"].textContent, "진단 정보 없음");
  assert.equal(get("runnerDiagnosticTime(null)"), "관측 기록 없음");
  assert.equal(get("runnerDiagnosticTime('not a date')"), "관측 기록 없음");
  await get("loadRunners(false)");
  assert.match(elements["runner-diagnostics-status"].textContent, /등록 정보를 찾을 수 없습니다/);
  assert.equal(elements["runner-diagnostics-facts"].children.length, 0);
});
