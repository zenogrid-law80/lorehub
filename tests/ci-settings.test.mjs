import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";
const app = await readFile(new URL("../web/app.js", import.meta.url), "utf8");
const fn = (start, end) => app.slice(app.indexOf(start), app.indexOf(end, app.indexOf(start)));
function setup(api) {
  const state = { repositoryConfigName: "game", repositoryConfigRequest: 0, repositoryConfigMode: "visual" };
  const elements = { "repository-config-branch": { value: "main" }, "repository-config-editor": { value: "" } };
  const context = vm.createContext({ state, elements, api, document: { getElementById() { return null; } }, t: value => value, validateCiBeforeSave: async () => true, csrfToken: () => "test-csrf", ciElement: () => ({}), rememberRepositoryBranch() {}, renderRepositoryConfig() {}, toast() {}, cloneCiModel: value => JSON.parse(JSON.stringify(value)) });
  vm.runInContext(fn("function applyCiStorageMetadata(", "function renderCiStorage("), context);
  vm.runInContext(fn("async function saveRepositoryConfig(", "function resetRepositoryConfig("), context);
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

test("visual serialization preserves parallelism for root and named pipelines", () => {
  const context = vm.createContext({});
  vm.runInContext(fn("function serializeCiModel(", "function openNewRepository("), context);
  const serialize = model => { context.model = model; return vm.runInContext("serializeCiModel(model)", context); };
  const job = { name: "build", stage: "build", script: ["echo ok"], timeout_seconds: 30 };
  const model = { stages: ["build"], jobs: [job], pipelines: [] };
  assert.doesNotMatch(serialize(model), /max_parallel_jobs/);
  model.max_parallel_jobs = 4;
  assert.match(serialize(model), /^max_parallel_jobs = 4\nstages =/);
  const named = { ...model, name: "build", category: "ci", runner_os: "linux", changes: ["src/**"], working_directory: "." };
  const output = serialize({ stages: [], jobs: [], pipelines: [named, { ...named, name: "serial", max_parallel_jobs: 1 }] });
  assert.equal(output.match(/max_parallel_jobs/g).length, 1);
  assert.ok(output.indexOf("max_parallel_jobs = 4") < output.indexOf("[[pipelines.jobs]]"));
  assert.ok(output.startsWith("[[pipelines]]"));
});

test("converting a manual pipeline preserves its parallelism and clears the root setting", () => {
  const draft = { max_parallel_jobs: 4, stages: ["build"], jobs: [], pipelines: [] };
  const state = { repositoryConfigEditing: true, repositoryConfigDraft: draft };
  const context = vm.createContext({ state, renderRepositoryConfigVisual() {} });
  vm.runInContext(fn("function addVisualPipeline(", "function deleteVisualPipeline("), context);
  vm.runInContext("addVisualPipeline()", context);
  assert.equal(draft.pipelines[0].max_parallel_jobs, 4);
  assert.equal(draft.max_parallel_jobs, undefined);
});

test("pipeline inspector edits parallelism on the root or selected named pipeline", () => {
  const node = () => ({ children: [], append(...items) { this.children.push(...items); } });
  const inspector = node();
  const context = vm.createContext({
    state: { repositoryConfigSelection: { type: "pipeline" }, repositoryConfigEditing: false },
    elements: { "repository-config-inspector": inspector, "repository-config-inspector-title": node() },
    document: { createElement: node }, t: value => value,
    configInspectorInput: (label, value, update, type, attributes) => ({ label, value, update, type, attributes }),
    configInspectorPipelineDependencies: node, configInspectorSelect: node, configInspectorTextarea: node,
  });
  vm.runInContext(fn("function renderConfigInspector(", "function configInspectorInput("), context);
  for (const legacy of [true, false]) {
    inspector.children = [];
    context.model = { max_parallel_jobs: 2, pipelines: [] };
    context.selected = { legacy, pipeline: { max_parallel_jobs: 4, changes: [], stages: [], jobs: [] } };
    vm.runInContext("renderConfigInspector(selected, model)", context);
    const field = inspector.children.find(item => item.label === "Maximum parallel jobs");
    assert.equal(field.value, legacy ? 2 : 4);
    assert.equal(field.attributes.max, 16);
    field.update("8");
    assert.equal(legacy ? context.model.max_parallel_jobs : context.selected.pipeline.max_parallel_jobs, 8);
  }
});

test("database metadata is distinct from the code revision and is sent with optimistic locking", async () => {
  const requests = [];
  const config = { revision: "unchanged-code", content: "old", configuration: { stages: [], jobs: [] }, source_mode: "db", lock_version: 7, config_revision_id: "v7", config_version: 7 };
  const { state, elements, get } = setup(async (url, options) => {
    if (!options) return config;
    requests.push(JSON.parse(options.body));
    return { ...config, content: "new", lock_version: 8, config_version: 8, config_revision_id: "v8" };
  });
  await get("loadRepositoryConfig()");
  assert.equal(state.repositoryConfigVersionId, "v7");
  get("setRepositoryConfigEditing(true)");
  state.repositoryConfigMode = "toml"; elements["repository-config-editor"].value = "new";
  await get("saveRepositoryConfig({ preventDefault() {} })");
  assert.deepEqual(requests[0], { branch: "main", expected_revision: "unchanged-code", expected_lock_version: 7, content: "new" });
  assert.equal(state.repositoryConfigRevision, "unchanged-code");
  assert.equal(state.repositoryConfigVersionId, "v8");
  assert.equal(state.repositoryConfigLockVersion, 8);
  assert.equal(state.repositoryConfigEditing, false);
});

test("a conflicting save preserves the draft and the original lock version", async () => {
  const { state, elements, get } = setup(async (url, options) => {
    if (options) throw new Error("CI settings changed");
    return { revision: "code", content: "saved", configuration: { jobs: [], stages: [] }, source_mode: "db", lock_version: 4, config_revision_id: "v4" };
  });
  await get("loadRepositoryConfig()"); get("setRepositoryConfigEditing(true)");
  state.repositoryConfigMode = "toml"; elements["repository-config-editor"].value = "unsaved";
  await get("saveRepositoryConfig({ preventDefault() {} })");
  assert.equal(state.repositoryConfigEditing, true);
  assert.equal(state.repositoryConfigContent, "saved");
  assert.equal(elements["repository-config-editor"].value, "unsaved");
  assert.equal(state.repositoryConfigLockVersion, 4);
});
