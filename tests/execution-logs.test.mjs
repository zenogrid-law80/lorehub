import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";

const source = await readFile(new URL("../web/execution-graph.js", import.meta.url), "utf8");
class Element {
  children = []; dataset = {}; style = {}; attributes = {}; listeners = {};
  classList = { add() {} }; scrollTop = 0; scrollHeight = 1000; clientHeight = 200;
  get isConnected() { return this.root || Boolean(this.parent?.isConnected); }
  append(...children) { for (const child of children) { child.parent = this; this.children.push(child); } }
  replaceChildren(...children) { this.children.forEach(child => { child.parent = null; }); this.children = []; this.append(...children); }
  contains(node) { return this === node || this.children.some(child => child.contains(node)); }
  setAttribute(key, value) { this.attributes[key] = value; }
  addEventListener(key, callback) { this.listeners[key] = callback; }
  getClientRects() { return this.isConnected ? [{}] : []; }
}
function setup() {
  const host = Object.assign(new Element(), { root: true });
  const frames = [], requests = [];
  const context = vm.createContext({
    I18N: { en: {}, ko: {}, "zh-CN": {} }, state: { runners: [] }, t: key => key,
    document: { activeElement: null, createElement: () => new Element() },
    textNode: (textContent, className) => Object.assign(new Element(), { textContent, className }),
    duration: () => "1s", statusLabel: status => status, renderExecutionAnalysis: () => new Element(),
    window: { requestAnimationFrame: callback => frames.push(callback) },
    api: path => new Promise((resolve, reject) => requests.push({ path, resolve, reject })),
  });
  vm.runInContext(source, context);
  const render = vm.runInContext("renderExecutionWorkspace", context);
  const detail = { pipeline: { id: "run", status: "failed" }, jobs: [{ id: "job", name: "compile", stage: "build", status: "failed", position: 0 }] };
  const draw = () => { render(host, detail, "drawer"); while (frames.length) frames.shift()(); };
  const find = predicate => {
    const visit = node => predicate(node) ? node : node.children.map(visit).find(Boolean);
    return visit(host);
  };
  return { detail, draw, requests, log: () => find(node => node.className === "run-log"), more: () => find(node => node.dataset.control === "more-logs"), note: () => find(node => node.className === "run-log-note") };
}
const settle = () => new Promise(resolve => setImmediate(resolve));
const rows = (after, count) => Array.from({ length: count }, (_, i) => ({ id: after + i + 1, stream: "stdout", content: `line ${after + i + 1}\n` }));

test("a slow log response updates the current inspector after a graph redraw", async () => {
  const view = setup(); view.draw();
  const original = view.log();
  view.draw();
  assert.notEqual(view.log(), original);
  assert.equal(view.requests.length, 1);
  view.requests[0].resolve(rows(0, 500)); await settle();
  assert.match(view.log().textContent, /line 500/);
  assert.equal(view.more().disabled, false);
});

test("redrawing never drains full log pages and a failed page retries its cursor", async () => {
  const view = setup(); view.draw();
  view.requests[0].resolve(rows(0, 500)); await settle();
  view.draw();
  assert.equal(view.requests.length, 1);
  view.more().listeners.click();
  assert.match(view.requests[1].path, /after=500&limit=500/);
  view.requests[1].reject(new Error("temporarily unavailable")); await settle();
  view.draw();
  assert.equal(view.requests.length, 2);
  assert.equal(view.more().textContent, "Retry logs");
  view.more().listeners.click();
  assert.equal(view.requests[2].path, view.requests[1].path);
  view.requests[2].resolve(rows(500, 1)); await settle();
  assert.match(view.log().textContent, /line 501/);
  assert.equal(view.more().hidden, true);
});

test("an oversized record is bounded without losing the next-page cursor", async () => {
  const view = setup(); view.draw();
  const page = rows(0, 500);
  page[499].content = "x".repeat(700000) + "tail";
  view.requests[0].resolve(page); await settle();
  assert.equal(view.log().textContent.length, 512000 + "[stdout] ".length);
  assert.ok(view.log().textContent.endsWith("tail"));
  assert.equal(view.note().textContent, "Showing the most recently loaded log lines");
  view.more().listeners.click();
  assert.match(view.requests[1].path, /after=500&limit=500/);
  view.requests[1].resolve([]); await settle();
});

test("retention changes discard old pages and their late responses", async () => {
  const view = setup(); view.draw();
  view.detail.pipeline.logs_pruned_at = "2026-09-26T00:00:00Z";
  view.draw();
  assert.equal(view.requests.length, 2);
  assert.match(view.requests[1].path, /after=0&limit=500/);
  view.requests[0].resolve([{ id: 1, stream: "stdout", content: "expired output" }]); await settle();
  assert.equal(view.log().textContent, "Loading job logs…");
  view.requests[1].resolve([{ id: 100, stream: "stdout", content: "retained output" }]); await settle();
  assert.equal(view.log().textContent, "[stdout] retained output");
  assert.match(view.note().textContent, /dynamic.logsPruned/);
});
