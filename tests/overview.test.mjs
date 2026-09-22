import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";
const source = await readFile(new URL("../web/overview.js", import.meta.url), "utf8");
function setup(api) {
  const context = vm.createContext({ api, state: { section: "overview", locale: "en" } });
  vm.runInContext(source, context);
  vm.runInContext("renderOverview = () => {}", context);
  return code => vm.runInContext(code, context);
}
test("Overview skips overlapping polls and ignores a response invalidated by navigation", async () => {
  let finish, calls = 0;
  const get = setup(() => { calls++; return new Promise(resolve => { finish = resolve; }); });
  const pending = get("loadOverview()");
  await get("loadOverview()");
  assert.equal(calls, 1);
  get("workspaceOverview.request++; workspaceOverview.loading=false");
  finish({ summary: { repositories: 99 } }); await pending;
  assert.equal(get("workspaceOverview.data"), null);
});
test("failed refresh clears stale activity and a retry recovers", async () => {
  let fail = false;
  const get = setup(async () => { if (fail) throw new Error("unavailable"); return { summary: { queued: 120 } }; });
  await get("loadOverview()");
  assert.equal(get("workspaceOverview.data.summary.queued"), 120);
  fail = true; await get("loadOverview()");
  assert.equal(get("workspaceOverview.data"), null);
  assert.ok(get("workspaceOverview.error"));
  assert.equal(get("workspaceOverview.loading"), false);
  fail = false; await get("loadOverview()");
  assert.equal(get("workspaceOverview.error"), "");
});

test("healthy snapshots and unavailable activity keep the attention panel hidden", () => {
  class Element {
    children = []; hidden = false;
    append(...items) { this.children.push(...items); }
    replaceChildren(...items) { this.children = items; }
    setAttribute() {}
    addEventListener() {}
  }
  const nodes = new Map();
  const context = vm.createContext({
    URLSearchParams, state: { section: "overview", locale: "en" },
    document: { createElement: () => new Element(), getElementById: id => {
      if (!nodes.has(id)) nodes.set(id, new Element()); return nodes.get(id);
    } },
    t: key => key, textNode: text => ({ textContent: text }), repositorySectionHash: section => `#${section}`,
  });
  vm.runInContext(source, context);
  vm.runInContext("renderOverviewRepositories=()=>{}; workspaceOverview.data={summary:{repositories:0,running:0,queued:0,online_runners:0,failed:0,waiting:0,link_errors:0}}; renderOverview()", context);
  assert.equal(nodes.get("overview-attention").hidden, true);
  assert.equal(nodes.get("overview-notice").hidden, true);
  vm.runInContext("workspaceOverview.data=null; workspaceOverview.error='unavailable'; renderOverview()", context);
  assert.equal(nodes.get("overview-attention").hidden, true);
  assert.equal(nodes.get("overview-notice").hidden, false);
});
