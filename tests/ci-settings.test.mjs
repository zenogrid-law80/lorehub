import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";
const app = await readFile(new URL("../web/app.js", import.meta.url), "utf8");
const fn = (start, end) => app.slice(app.indexOf(start), app.indexOf(end, app.indexOf(start)));
function setup(api) {
  const state = { repositoryConfigName: "game", repositoryConfigRequest: 0, repositoryConfigMode: "visual" };
  const elements = { "repository-config-branch": { value: "main" }, "repository-config-editor": { value: "" } };
  const context = vm.createContext({ state, elements, api, ciElement: () => ({}), rememberRepositoryBranch() {}, renderRepositoryConfig() {}, toast() {}, cloneCiModel: value => JSON.parse(JSON.stringify(value)) });
  vm.runInContext(fn("const DEFAULT_CI_CONFIG", "const I18N"), context);
  vm.runInContext(fn("async function loadRepositoryConfig()", "function renderRepositoryConfig()"), context);
  vm.runInContext(fn("function setRepositoryConfigEditing(", "async function setRepositoryConfigMode("), context);
  return { state, elements, get: code => vm.runInContext(code, context) };
}
test("missing root configuration offers a draft template without writing the repository", async () => {
  let calls = 0;
  const { state, elements, get } = setup(async () => { calls++; return { revision: "head", content: null, configuration: null, is_link_source: false }; });
  await get("loadRepositoryConfig()");
  assert.equal(state.repositoryConfigStatus, "ready");
  assert.equal(state.repositoryConfigContent, null);
  get("setRepositoryConfigEditing(true)");
  assert.equal(state.repositoryConfigEditing, true);
  assert.equal(state.repositoryConfigDraft.jobs[0].stage, "build");
  assert.match(elements["repository-config-editor"].value, /echo Configure your CI job/);
  assert.equal(state.repositoryConfigRevision, "head");
  assert.equal(calls, 1);
});
test("source repositories cannot start a configuration draft", async () => {
  const { state, get } = setup(async () => ({ revision: null, content: null, configuration: null, is_link_source: true }));
  await get("loadRepositoryConfig()");
  assert.equal(state.repositoryConfigStatus, "link-source");
  get("setRepositoryConfigEditing(true)");
  assert.equal(state.repositoryConfigEditing, false);
  assert.equal(state.repositoryConfigDraft, null);
});
test("a repository containing links remains editable when it is not a source", async () => {
  const { state, get } = setup(async () => ({ revision: "root-head", content: null, configuration: null, is_link_source: false, has_links: true }));
  await get("loadRepositoryConfig()");
  assert.equal(state.repositoryConfigStatus, "ready");
  get("setRepositoryConfigEditing(true)");
  assert.equal(state.repositoryConfigEditing, true);
});
test("access failures do not become a missing-file template", async () => {
  const { state, get } = setup(async () => { throw new Error("permission denied"); });
  await get("loadRepositoryConfig()");
  assert.equal(state.repositoryConfigStatus, "error");
  get("setRepositoryConfigEditing(true)");
  assert.equal(state.repositoryConfigEditing, false);
});
test("a late missing-file response cannot replace another branch's configuration", async () => {
  let finish;
  const { state, elements, get } = setup(() => new Promise(resolve => { finish = resolve; }));
  const pending = get("loadRepositoryConfig()");
  elements["repository-config-branch"].value = "release";
  state.repositoryConfigContent = "existing release configuration";
  finish({ revision: "old", content: null, configuration: null, is_link_source: false });
  await pending;
  assert.equal(state.repositoryConfigContent, "existing release configuration");
});
