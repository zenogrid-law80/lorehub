import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";
const source = await readFile(new URL("../web/operations.js", import.meta.url), "utf8");
class Element {
  children = []; attributes = {}; textContent = ""; hidden = false;
  append(...nodes) { this.children.push(...nodes); }
  replaceChildren(...nodes) { this.children = nodes; }
  setAttribute(key, value) { this.attributes[key] = value; }
}
function setup(api = async () => snapshot()) {
  const nodes = new Map(), notices = [];
  const node = id => { if (!nodes.has(id)) nodes.set(id, new Element()); return nodes.get(id); };
  const state = { user: { id: "admin", role: "admin" }, section: "pipelines", locale: "en" };
  const context = vm.createContext({ state, api, AbortController, window: { setTimeout, clearTimeout },
    document: { getElementById: node, createTextNode: text => ({ textContent: text }) },
    toast: (...args) => notices.push(args), localeTag: () => "en-US", mt: key => key,
    mn: (tag, className, textContent = "") => Object.assign(new Element(), { tag, className, textContent }),
  });
  vm.runInContext(source, context);
  const get = code => vm.runInContext(code, context);
  const observe = value => { context.snapshot = value; get("observeOperationsAlerts(snapshot); renderOperationsAlertBanner()"); };
  return { state, context, get, observe, notices, node };
}
const snapshot = (seconds = 0, overrides = {}) => ({
  observed_at: new Date(Date.UTC(2026, 8, 25, 0, 0, seconds)).toISOString(),
  queue: { queued: 0, oldest_wait_seconds: null, expired_leases: 0 }, runners: [], repositories: [], ...overrides,
});
const repository = (overrides = {}) => ({ resource_id: "repo", name: "Game", state: "error", consecutive_failures: 3, link_errors: 0, ...overrides });

test("thresholds detect five issue categories and repeated observations do not notify again", () => {
  const { observe, get, notices } = setup();
  const failure = snapshot(0, {
    queue: { queued: 1, oldest_wait_seconds: 300, expired_leases: 1 },
    runners: [{ os: "windows", online: 0, offline: 1 }], repositories: [repository({ link_errors: 1 })],
  });
  observe(failure);
  assert.equal(get("operationsAlerts.active.size"), 5);
  assert.equal(get("operationsAlerts.events.length"), 5);
  assert.equal(notices.length, 1);
  observe({ ...failure, observed_at: snapshot(30).observed_at });
  assert.equal(get("operationsAlerts.events.length"), 5);
  assert.equal(notices.length, 1);
  observe(snapshot(60, { runners: [{ os: "windows", online: 1, offline: 0 }], repositories: [repository({ state: "ok", consecutive_failures: 0 })] }));
  assert.equal(get("operationsAlerts.active.size"), 0);
  assert.equal(get("operationsAlerts.events.filter(event => event.event === 'alertRecovered').length"), 5);
  assert.equal(notices.length, 2);
  observe({ ...failure, observed_at: snapshot(90).observed_at });
  assert.equal(notices.length, 3);
});

test("short waits, first failures, unknown state and intentionally stopped runners do not trigger", () => {
  const { observe, get, notices } = setup();
  observe(snapshot(0, {
    queue: { queued: 1, oldest_wait_seconds: 299, expired_leases: 0 },
    runners: [{ os: "windows", online: 0, offline: 0, stopped: 3 }],
    repositories: [repository({ consecutive_failures: 2 }), repository({ resource_id: "stale", state: "stale" })],
  }));
  assert.equal(get("operationsAlerts.active.size"), 0);
  assert.equal(notices.length, 0);
});

test("missing rows, stale checks, and stopped runners do not falsely confirm recovery", () => {
  const { observe, get, notices } = setup();
  observe(snapshot(0, { repositories: [repository()], runners: [{ os: "linux", online: 0, offline: 1 }] }));
  observe(snapshot(30));
  assert.equal(get("operationsAlerts.active.size"), 2);
  assert.equal(get("[...operationsAlerts.active.values()].every(alert => !alert.confirmed)"), true);
  observe(snapshot(60, { repositories: [repository({ state: "stale", consecutive_failures: 0 })], runners: [{ os: "linux", online: 0, offline: 0, stopped: 1 }] }));
  assert.equal(get("operationsAlerts.active.size"), 2);
  assert.equal(notices.length, 1);
});

test("failed requests retain incidents as stale and only successful observations resolve them", async () => {
  let mode = "failure";
  const { get, notices, node } = setup(async () => {
    if (mode === "network") throw new Error("offline");
    return snapshot(mode === "failure" ? 0 : 60, { repositories: [repository(mode === "recovered" ? { state: "ok", consecutive_failures: 0 } : {})] });
  });
  await get("loadOperations()");
  mode = "network"; await get("loadOperations()");
  assert.equal(get("operations.snapshot"), null);
  assert.equal(get("operationsAlerts.active.size"), 1);
  assert.match(node("operations-alert-summary").textContent, /unavailable/);
  assert.equal(notices.length, 1);
  mode = "recovered"; await get("loadOperations()");
  assert.equal(get("operationsAlerts.active.size"), 0);
  assert.equal(notices.length, 2);
});

test("read action clears unread changes without dismissing unresolved incidents", () => {
  const { observe, get, node, state } = setup();
  observe(snapshot(0, { repositories: [repository()] }));
  assert.equal(node("operations-alert-banner").hidden, false);
  node("operations-alert-read").onclick();
  assert.equal(get("operationsAlerts.unread"), 0);
  assert.equal(node("operations-alert-banner").hidden, false);
  assert.equal(node("operations-alert-read").disabled, true);
  state.locale = "ko"; get("renderOperationsAlertBanner()");
  assert.match(node("operations-alert-summary").textContent, /미해결/);
  observe(snapshot(30, { repositories: [repository({ state: "ok" })] }));
  assert.equal(node("operations-alert-banner").hidden, false);
  node("operations-alert-read").onclick();
  assert.equal(node("operations-alert-banner").hidden, true);
});

test("authorization errors clear stored alerts and non-admins cannot fetch operations", async () => {
  let requests = 0;
  const { observe, get, state, node } = setup(async () => { requests++; throw Object.assign(new Error("forbidden"), { status: 403 }); });
  observe(snapshot(0, { repositories: [repository()] }));
  await get("loadOperations()");
  assert.equal(get("operationsAlerts.active.size"), 0);
  assert.equal(get("operationsAlerts.events.length"), 0);
  assert.equal(get("operations.forbidden"), true);
  assert.equal(node("operations-alert-banner").hidden, true);
  state.user.role = "user";
  await get("loadOperations()");
  assert.equal(requests, 1);
});

test("a response from the previous account cannot create notifications", async () => {
  let finish;
  const { get, state, notices } = setup(() => new Promise(resolve => { finish = resolve; }));
  const pending = get("loadOperations()");
  state.user = { id: "other", role: "admin" };
  finish(snapshot(0, { repositories: [repository()] })); await pending;
  assert.equal(get("operationsAlerts.active.size"), 0);
  assert.equal(get("operations.snapshot"), null);
  assert.equal(notices.length, 0);
});

test("old or invalid snapshots cannot create recovery events and history stays bounded", () => {
  const { observe, get, notices } = setup();
  for (let i = 0; i < 60; i++) observe(snapshot(i, { repositories: [repository({ state: i % 2 ? "ok" : "error" })] }));
  assert.equal(get("operationsAlerts.events.length"), 50);
  assert.equal(get("operationsAlerts.unread"), 50);
  observe(snapshot(58, { repositories: [repository()] }));
  observe({ ...snapshot(), observed_at: "invalid" });
  assert.equal(notices.length, 60);
  assert.equal(get("operationsAlerts.active.size"), 0);
});

test("alert panel renders literal subject text and reports missing observations", () => {
  const { observe, get, context } = setup();
  observe(snapshot(0, { repositories: [repository({ name: "<img onerror=alert(1)>" })] }));
  observe(snapshot(30));
  const page = new Element(); context.page = page;
  get("renderOperationsAlertPanel(page)");
  const text = node => [node.textContent, ...node.children?.flatMap(child => text(child)) || []];
  const labels = text(page);
  assert.ok(labels.includes("<img onerror=alert(1)>"));
  assert.ok(labels.includes("Not confirmed in this snapshot"));
  assert.equal(page.children.length, 2);
});
