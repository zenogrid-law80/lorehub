import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";

const source = await readFile(new URL("../web/repository-context.js", import.meta.url), "utf8");
function setup(api) {
  const nodes = new Map();
  const node = id => {
    if (!nodes.has(id)) nodes.set(id, { value: "", hidden: false, disabled: false, textContent: "", reset() {}, focus() {}, showModal() { this.open = true; }, close() { this.open = false; } });
    return nodes.get(id);
  };
  const navigations = [], toasts = [];
  const context = vm.createContext({
    state: { repositoryScope: "lores://fixture/root", repositoryBranch: "release", section: "repository-links", locale: "ko" },
    document: { getElementById: node }, api, csrfToken: () => "csrf-test",
    repositoryName: url => url.split("/").at(-1), toast: (...args) => toasts.push(args),
    navigations,
  });
  vm.runInContext(source, context);
  const run = code => vm.runInContext(code, context);
  run("navigateRepositorySection = (...args) => navigations.push(args); repositoryBranchOptions.set(state.repositoryScope, [{ name: 'release', revision: 'a'.repeat(64) }]); openRepositoryBranchDialog()");
  node("repository-branch-name").value = "feature/release";
  return { node, run, navigations, toasts, submit: () => run("submitRepositoryBranch({ preventDefault() {} })") };
}

test("branch creation sends the captured base revision with CSRF and selects the published branch", async () => {
  const requests = [];
  const { submit, node, navigations, run } = setup(async (url, options) => {
    requests.push({ url, ...options }); return { name: "feature/release", revision: "b".repeat(64) };
  });
  await submit();
  assert.equal(requests[0].url, "/api/v1/repositories/root/branches");
  assert.equal(requests[0].headers["X-CSRF-Token"], "csrf-test");
  assert.deepEqual(JSON.parse(requests[0].body), { name: "feature/release", from_branch: "release", expected_revision: "a".repeat(64) });
  assert.deepEqual(Array.from(navigations[0]), ["repository-links", "lores://fixture/root", "feature/release"]);
  assert.equal(node("new-repository-branch-dialog").open, false);
  assert.equal(run("repositoryBranchOptions.get(state.repositoryScope).at(-1).revision"), "b".repeat(64));
});

test("duplicate submits are blocked and late success cannot navigate away from another repository", async () => {
  let resolve, requests = 0;
  const { submit, node, run, navigations } = setup(() => { requests++; return new Promise(done => { resolve = done; }); });
  const pending = submit();
  assert.equal(node("repository-branch-cancel").disabled, true);
  await submit();
  assert.equal(requests, 1);
  run("state.repositoryScope = 'lores://fixture/other'");
  resolve({ name: "feature/release", revision: "b".repeat(64) });
  await pending;
  assert.equal(navigations.length, 0);
  assert.equal(node("repository-branch-submit").disabled, false);
});

test("partial failure preserves the draft and displays the failure without reporting success", async () => {
  const { submit, node, navigations, toasts } = setup(async () => { throw new Error("Source denied"); });
  await submit();
  assert.equal(node("new-repository-branch-dialog").open, true);
  assert.equal(node("repository-branch-name").value, "feature/release");
  assert.match(node("repository-branch-error").textContent, /Source denied/);
  assert.equal(node("repository-branch-error").hidden, false);
  assert.equal(node("repository-branch-submit").disabled, false);
  assert.equal(navigations.length, 0);
  assert.equal(toasts.length, 0);
});

test("empty, unchanged and control-character names never submit", async () => {
  let requests = 0;
  const { submit, node } = setup(async () => { requests++; });
  for (const name of ["   ", "release", "bad\u0001name"]) {
    node("repository-branch-name").value = name;
    await submit();
    assert.equal(node("repository-branch-error").hidden, false);
  }
  assert.equal(requests, 0);
});
