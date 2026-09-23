import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";
const source = await readFile(new URL("../web/repository-context.js", import.meta.url), "utf8");
const app = await readFile(new URL("../web/app.js", import.meta.url), "utf8");
const fn = (name, next) => app.slice(app.indexOf(name), app.indexOf(next, app.indexOf(name)));
function setup(extra = {}) {
  const context = vm.createContext({ URLSearchParams, state: { repositoryScope: "", repositories: [] }, ...extra });
  vm.runInContext(source, context);
  return { context, get: code => vm.runInContext(code, context) };
}

test("repository routes round-trip complete URLs and keep global pages unscoped", () => {
  const { context, get } = setup();
  const url = "lores://host:41337/demo?value=a&b=한글#revision";
  context.scope = url;
  assert.equal(get("repositoryRoute(repositorySectionHash('pipelines', scope)).repository"), url);
  assert.equal(get("repositoryRoute(repositorySectionHash('graphs', scope)).section"), "graphs");
  assert.equal(get("repositorySectionHash('runners', scope)"), "#runners");
  assert.equal(get("repositoryRoute('#operations?repository=anything').repository"), "");
  assert.equal(get("repositoryRoute('#pipelines').repository"), "");
});

test("repository configuration candidates match the full URL and never fall back to another repository", () => {
  const { get } = setup({ state: { repositoryScope: "lores://one/demo", repositories: [{ name: "demo", url: "lores://one/demo" }, { name: "demo", url: "lores://two/demo" }] } });
  assert.equal(get("scopedRepositories().length"), 1);
  assert.equal(get("scopedRepositories()[0].url"), "lores://one/demo");
  get("state.repositoryScope='lores://missing/demo'");
  assert.equal(get("scopedRepositories().length"), 0);
});

function historySetup(api) {
  const { context, get } = setup({
    state: { repositoryScope: "lores://host/one", pipelines: [], pipelineRequest: 0, pipelineLoading: false, pipelineNextBefore: null, updatedAt: {} },
    api, elements: { "refresh-button": {}, "load-more-pipelines": {} },
    renderPipelines() {}, renderUpdatedLabels() {}, toast() {}, t: x => x,
  });
  vm.runInContext(fn("async function loadPipelines(", "async function loadRepositories("), context);
  return { context, get };
}

test("new runs use the CI branch and cannot silently switch to main if it was deleted", async () => {
  const errors = [];
  let selected = "";
  const { context, get } = setup({
    state: { repositoryScope: "lores://host/game", repositoryBranch: "release", repositories: [{ url: "lores://host/game", name: "game" }] },
    elements: {
      "repository-url": { value: "lores://host/game" },
      branch: { value: "", replaceChildren() {}, append() {} },
      revision: { value: "" },
      "pipeline-name": { dataset: {}, replaceChildren() {} },
      "run-pipeline-button": {},
    },
    Option: function(text, value) { return { text, value, dataset: {} }; },
    api: async () => [{ name: "main", revision: "main-revision" }, { name: "release", revision: "release-revision" }],
    selectPipelineBranch: async () => { selected = get("elements.branch.value"); },
    toast: message => errors.push(message), t: key => key,
  });
  vm.runInContext(fn("async function loadPipelineBranches(", "async function selectPipelineBranch("), context);
  await get("loadPipelineBranches('release')");
  assert.equal(selected, "release");
  assert.equal(get("elements.branch.disabled"), false);
  selected = "";
  context.api = async () => [{ name: "main", revision: "main-revision" }];
  await get("loadPipelineBranches('release')");
  assert.equal(selected, "");
  assert.equal(get("elements.branch.disabled"), true);
  assert.equal(get("elements['run-pipeline-button'].disabled"), true);
  assert.equal(errors.length, 1);
});

test("Overview requests only five recent runs and never appends history", async () => {
  const requests = [];
  const { get } = historySetup(async url => {
    requests.push(url);
    return { pipelines: [{ id: "recent" }], next_before: "older" };
  });
  get("state.section='overview'; state.repositoryScope=''");
  await get("loadPipelines(false)");
  assert.equal(new URL(requests[0], "http://test").searchParams.get("limit"), "5");
  assert.equal(get("elements['load-more-pipelines'].hidden"), true);
  await get("loadPipelines(false, true)");
  assert.equal(requests.length, 1);
});

test("recent visits remember full repository URLs and branches, deduplicate and isolate accounts", () => {
  const { get, storage } = selectionSetup();
  get("rememberRepositorySelection('lores://host/one'); repositoryBranchSelections.set('lores://host/two', 'release'); rememberRepositorySelection('lores://host/two')");
  assert.deepEqual(JSON.parse(JSON.stringify(get("recentRepositoryVisits()"))), [
    { repository: "lores://host/two", branch: "release" }, { repository: "lores://host/one", branch: "" },
  ]);
  get("rememberRepositorySelection('lores://host/one')");
  assert.equal(get("recentRepositoryVisits().length"), 2);
  assert.equal(get("recentRepositoryVisits()[0].repository"), "lores://host/one");
  get("state.repositories=state.repositories.filter(item => item.url.endsWith('/two')); reconcileRepositorySelection()");
  assert.equal(get("recentRepositoryVisits().length"), 1);
  assert.equal(JSON.parse(storage.get('lorehub.repository-visits:user-one')).length, 1);
  get("state.user.id='another'");
  assert.equal(get("recentRepositoryVisits().length"), 0);
});

test("history pagination includes repository scope and preserves loaded older pages", async () => {
  const requests = [];
  const { context, get } = historySetup(async url => {
    requests.push(new URL(url, "http://test"));
    return requests.length === 1 ? { pipelines: [{ id: "new" }], next_before: "new" } : { pipelines: [{ id: "old" }], next_before: null };
  });
  await get("loadPipelines(false)");
  await get("loadPipelines(false, true)");
  assert.deepEqual(Array.from(get("state.pipelines"), p => p.id), ["new", "old"]);
  for (const request of requests) assert.equal(request.searchParams.get("repository_url"), "lores://host/one");
  assert.equal(requests[1].searchParams.get("before"), "new");
  assert.equal(get("state.pipelineHasOlderPages"), true);
  let polls = 0;
  Object.assign(get("state"), { section: "pipelines", selectedId: null });
  Object.assign(context, { document: { hidden: false }, isManagement: () => false, loadPipelines: async () => { polls++; } });
  vm.runInContext(fn("async function refreshActiveViews()", "function csrfToken()"), context);
  await get("refreshActiveViews()");
  assert.equal(polls, 0);
});

test("a late history response from the previous repository cannot replace the selected repository", async () => {
  let finishOld;
  const { get } = historySetup(async url => {
    if (url.includes("%2Fone")) return new Promise(resolve => { finishOld = resolve; });
    return { pipelines: [{ id: "two-run" }], next_before: null };
  });
  const old = get("loadPipelines(false)");
  get("state.repositoryScope='lores://host/two'; state.pipelineRequest++; state.pipelineLoading=false;");
  await get("loadPipelines(false)");
  finishOld({ pipelines: [{ id: "one-run" }], next_before: "old-cursor" });
  await old;
  assert.deepEqual(Array.from(get("state.pipelines"), p => p.id), ["two-run"]);
  assert.equal(get("state.pipelineNextBefore"), null);
  assert.equal(get("state.pipelineLoading"), false);
});

test("declining to discard CI edits restores the repository URL and keeps the draft", async () => {
  const replacements = [];
  const { context, get } = setup({
    state: { section: "ci-settings", repositoryScope: "lores://host/one", repositoryConfigEditing: true, repositoryConfigDraft: { unchanged: true } },
    window: { location: { hash: "#ci-settings?repository=lores%3A%2F%2Fhost%2Ftwo" } },
    availableSections: () => ["ci-settings"], discardRepositoryConfigEdit: () => false, renderRepositoryContext() {},
    history: { replaceState: (_, __, url) => replacements.push(url) },
    document: { getElementById: () => ({}) },
  });
  context.renderRepositoryContext = () => {};
  vm.runInContext(fn("async function showSection(", "function updateSectionSearch("), context);
  await get("showSection('ci-settings')");
  assert.equal(get("state.repositoryScope"), "lores://host/one");
  assert.equal(get("state.repositoryConfigDraft.unchanged"), true);
  assert.equal(replacements[0], "#ci-settings?repository=lores%3A%2F%2Fhost%2Fone");
});

test("history URL restores exact filters and sends literal search through every page", async () => {
  const requests = [];
  const { context, get } = historySetup(async url => {
    requests.push(new URL(url, "http://test"));
    return { pipelines: [], next_before: requests.length === 1 ? "cursor" : null };
  });
  context.document = { querySelectorAll: () => [] };
  context.elements["pipeline-search"] = {};
  context.hash = "#pipelines?branch=release%2F2026&pipeline_name=Build+%26+Test&status=active&q=100%25_%ED%95%9C%EA%B8%80";
  get("state.section='pipelines'; restorePipelineHistoryFilters(hash)");
  assert.equal(get("state.query"), "100%_한글");
  assert.equal(get("state.filter"), "running");
  await get("loadPipelines(false)");
  await get("loadPipelines(false, true)");
  for (const request of requests) {
    assert.equal(request.searchParams.get("branch"), "release/2026");
    assert.equal(request.searchParams.get("pipeline_name"), "Build & Test");
    assert.equal(request.searchParams.get("q"), "100%_한글");
    assert.equal(request.searchParams.get("status"), "active");
  }
  assert.equal(get("state.pipelineNameFilter"), "Build & Test");
  get("state.section='graphs'; restorePipelineHistoryFilters(hash)");
  assert.equal(get("pipelineHistoryParameters().has('q')"), false);
  assert.equal(get("state.pipelineBranchFilter"), "");
});

test("typing resets pagination immediately and debounces requests without letting polling bypass it", async () => {
  const callbacks = new Map();
  let serial = 0, loads = 0;
  const urls = [];
  const { context, get } = historySetup(async () => { loads++; return { pipelines: [], next_before: null }; });
  context.window = { location: { hash: "#pipelines" }, clearTimeout: id => callbacks.delete(id), setTimeout: fn => { const id = ++serial; callbacks.set(id, fn); return id; } };
  context.history = { replaceState: (_, __, url) => urls.push(url) };
  context.document = { hidden: false };
  context.renderRepositoryContext = () => {};
  context.isManagement = () => false;
  vm.runInContext(fn("async function refreshActiveViews()", "function csrfToken()"), context);
  get("Object.assign(state, {section:'pipelines', query:'first', filter:'failed', pipelineNextBefore:'old', pipelineHasOlderPages:true}); searchPipelineHistory(250)");
  assert.equal(get("state.pipelineNextBefore"), null);
  assert.equal(get("state.pipelineHasOlderPages"), false);
  get("state.query='second'; searchPipelineHistory(250)");
  assert.equal(callbacks.size, 1);
  await get("refreshActiveViews()");
  assert.equal(loads, 0);
  assert.ok(urls.at(-1).includes("q=second"));
  assert.ok(urls.at(-1).includes("status=failed"));
  assert.ok(urls.at(-1).includes("repository=lores%3A%2F%2Fhost%2Fone"));
  get("invalidatePipelineHistory()");
  assert.equal(callbacks.size, 0);
});

test("a slow previous search cannot overwrite results or cursors for the new search", async () => {
  let resolveOld;
  const { context, get } = historySetup(async url => {
    if (new URL(url, "http://test").searchParams.get("q") === "old") return new Promise(resolve => { resolveOld = resolve; });
    return { pipelines: [{ id: "new-match" }], next_before: null };
  });
  context.window = { clearTimeout() {} };
  get("state.section='pipelines'; state.query='old'");
  const pending = get("loadPipelines(false)");
  get("state.query='new'; invalidatePipelineHistory()");
  await get("loadPipelines(false)");
  resolveOld({ pipelines: [{ id: "old-match" }], next_before: "obsolete" });
  await pending;
  assert.deepEqual(Array.from(get("state.pipelines"), p => p.id), ["new-match"]);
  assert.equal(get("state.pipelineNextBefore"), null);
});

test("repository menu links preserve the branch while global routes clear scope", () => {
  const { get } = setup({ state: { repositoryScope: "lores://host/game", repositoryBranch: "release/한국어 & fixes", section: "repository-links" } });
  for (const section of ["repository-links", "pipelines", "graphs", "ci-settings"]) {
    const route = get(`repositoryRoute(repositorySectionHash('${section}', state.repositoryScope))`);
    assert.equal(route.repository, "lores://host/game");
    assert.equal(route.branch, "release/한국어 & fixes");
  }
  assert.equal(get("repositorySectionHash('pipelines', '')"), "#pipelines");
  assert.equal(get("repositorySectionHash('repositories', state.repositoryScope)"), "#repositories");
  assert.equal(get("repositoryNavigationValue('pipelines', '')"), "pipelines");
  assert.equal(get("repositoryNavigationValue('pipelines', state.repositoryScope)"), "repository:pipelines");
});

function branchSetup(api = async () => []) {
  const window = { location: { hash: "#repository-links?repository=lores%3A%2F%2Fhost%2Fgame&branch=release" } };
  const { context, get } = setup({
    state: { repositories: [], repositoryScope: "lores://host/game", repositoryBranch: "release", section: "repository-links" },
    window, history: { state: {}, replaceState(_, __, hash) { window.location.hash = hash; } },
    api, repositoryName: url => url.split('/').at(-1), toast() {},
  });
  context.renderRepositoryContext = () => {};
  return { context, get };
}

test("branch selection uses the URL branch and remembers choices separately for each repository", () => {
  const { get } = branchSetup();
  assert.equal(get("selectRepositoryBranch([{name:'main'}, {name:'release'}]).name"), "release");
  get("state.repositoryScope='lores://host/developer'; state.repositoryBranch='main'; selectRepositoryBranch([{name:'main'}])");
  assert.equal(get("repositoryBranchFor('lores://host/game')"), "release");
  assert.equal(get("repositoryBranchFor('lores://host/developer')"), "main");
  assert.equal(get("repositoryRoute(repositorySectionHash('ci-settings','lores://host/game')).branch"), "release");
});

test("deleted branches fall back consistently in state, URL, and repository links", () => {
  const { context, get } = branchSetup();
  assert.equal(get("selectRepositoryBranch([{name:'main'}]).name"), "main");
  assert.equal(get("state.repositoryBranch"), "main");
  assert.equal(get("repositoryRoute(window.location.hash).branch"), "main");
  assert.equal(get("repositoryRoute(repositorySectionHash('repository-links', state.repositoryScope)).branch"), "main");
  get("selectRepositoryBranch([])");
  assert.equal(get("state.repositoryBranch"), "");
  assert.ok(!context.window.location.hash.includes('branch='));
});

test("late navigation branch responses cannot overwrite a newer repository", async () => {
  let finish;
  const { get } = branchSetup(() => new Promise(resolve => { finish = resolve; }));
  const pending = get("loadRepositoryNavigationBranches()");
  get("state.repositoryScope='lores://host/other'; state.repositoryBranch='other-branch'");
  finish([{name:'main'}]);
  assert.equal(await pending, false);
  assert.equal(get("state.repositoryBranch"), "other-branch");
});

test("declining a branch change preserves CI edits, the old branch URL, and menu selection", async () => {
  const { context, get } = branchSetup();
  const mobile = {};
  Object.assign(context, { availableSections: () => ["ci-settings"], discardRepositoryConfigEdit: () => false, document: { getElementById: () => mobile } });
  get("state.section='ci-settings'; state.repositoryConfigEditing=true; state.repositoryConfigDraft={kept:true}; window.location.hash='#ci-settings?repository=lores%3A%2F%2Fhost%2Fgame&branch=main'");
  vm.runInContext(fn("async function showSection(", "function updateSectionSearch("), context);
  await get("showSection('ci-settings')");
  assert.equal(get("state.repositoryBranch"), "release");
  assert.equal(get("state.repositoryConfigDraft.kept"), true);
  assert.equal(get("repositoryRoute(window.location.hash).branch"), "release");
  assert.equal(mobile.value, "repository:ci-settings");
});

test("history keeps an explicitly selected archived branch instead of showing main", async () => {
  const { get } = branchSetup(async () => [{name:'main'}]);
  get("state.section='pipelines'; state.repositoryBranch='archived'; window.location.hash='#pipelines?repository=lores%3A%2F%2Fhost%2Fgame&branch=archived'");
  assert.equal(await get("loadRepositoryNavigationBranches()"), true);
  assert.equal(get("state.repositoryBranch"), "archived");
  assert.equal(get("repositoryRoute(window.location.hash).branch"), "archived");
});

function selectionSetup({ hash = '#overview', saved, ...extra } = {}) {
  const storage = new Map(saved === undefined ? [] : [['lorehub.repository-selection:user-one', saved]]);
  const { context, get } = setup({
    state: { user: { id: 'user-one' }, section: 'overview', repositoryScope: '', selectedRepository: '', repositories: [
      { name: 'One', url: 'lores://host/one' }, { name: 'Two', url: 'lores://host/two' },
    ] },
    window: { location: { hash } },
    localStorage: { getItem: key => storage.get(key), setItem: (key, value) => storage.set(key, value), removeItem: key => storage.delete(key) },
    ...extra,
  });
  return { context, get, storage };
}

test('restoring a sidebar selection leaves the global page and history filters unscoped', () => {
  const { get } = selectionSetup({ saved: JSON.stringify({ repository: 'lores://host/one', branch: 'release' }) });
  get('reconcileRepositorySelection()');
  assert.equal(get('selectedRepository()'), 'lores://host/one');
  assert.equal(get("repositoryBranchFor(selectedRepository())"), 'release');
  assert.equal(get('state.repositoryScope'), '');
  assert.equal(get('pipelineHistoryParameters().has("repository_url")'), false);
  assert.equal(get('window.location.hash'), '#overview');
  assert.equal(get('repositoryNavigationValue()'), 'overview');
});

test('an explicit repository URL takes precedence over the remembered selection', () => {
  const { get, storage } = selectionSetup({
    saved: JSON.stringify({ repository: 'lores://host/one', branch: 'old' }),
    hash: '#repository-links?repository=lores%3A%2F%2Fhost%2Ftwo&branch=release',
  });
  get('reconcileRepositorySelection()');
  assert.equal(get('selectedRepository()'), 'lores://host/two');
  assert.equal(get('repositoryBranchFor(selectedRepository())'), 'release');
  assert.equal(JSON.parse(storage.get('lorehub.repository-selection:user-one')).repository, 'lores://host/two');
});

test('removed and inaccessible repositories are cleared without selecting a replacement', () => {
  const { get, storage } = selectionSetup({ saved: JSON.stringify({ repository: 'lores://host/missing' }) });
  get('reconcileRepositorySelection()');
  assert.equal(get('selectedRepository()'), '');
  assert.equal(storage.size, 0);
  get("rememberRepositorySelection('lores://host/one'); state.repositories = []; reconcileRepositorySelection()");
  assert.equal(get('selectedRepository()'), '');
  assert.equal(storage.size, 0);
});

test('stored selections are isolated by account and invalid or unavailable storage is harmless', () => {
  const { get } = selectionSetup({ saved: JSON.stringify({ repository: 'lores://host/one' }) });
  get("state.user.id = 'user-two'; reconcileRepositorySelection()");
  assert.equal(get('selectedRepository()'), '');
  const broken = selectionSetup({ saved: '{broken' });
  broken.get('reconcileRepositorySelection()');
  assert.equal(broken.get('selectedRepository()'), '');
  const blocked = selectionSetup({ localStorage: { getItem() { throw Error('blocked'); }, setItem() { throw Error('blocked'); }, removeItem() { throw Error('blocked'); } } });
  blocked.get("reconcileRepositorySelection(); rememberRepositorySelection('lores://host/one')");
  assert.equal(blocked.get('selectedRepository()'), 'lores://host/one');
});

test('switching repositories keeps a scoped page and opens links from global pages', () => {
  const calls = [];
  const { context, get } = selectionSetup();
  context.navigateRepositorySection = (...args) => calls.push(args);
  get("state.selectedRepository='lores://host/one'; state.repositoryScope='lores://host/one'; state.section='repository-links'; switchRepository('lores://host/two')");
  assert.deepEqual(calls.pop(), ['repository-links', 'lores://host/two']);
  assert.equal(get('selectedRepository()'), 'lores://host/one', 'selection is not committed before unsaved-edit checks');
  get("state.repositoryScope=''; state.section='pipelines'; switchRepository('lores://host/two')");
  assert.deepEqual(calls.pop(), ['repository-links', 'lores://host/two']);
  get("switchRepository('lores://host/missing')");
  assert.equal(calls.length, 0);
});

test('repository search matches names and complete URLs without affecting page filters', () => {
  const { get } = selectionSetup();
  assert.equal(get("matchingRepositories(' ONE ')[0].url"), 'lores://host/one');
  assert.equal(get("matchingRepositories('HOST/TWO')[0].name"), 'Two');
  assert.equal(get("matchingRepositories('missing').length"), 0);
  assert.equal(get('state.repositoryScope'), '');
});

test('global navigation keeps local menu links available while highlighting only the global page', () => {
  const { context, get } = selectionSetup();
  const links = ['global', 'repository'].map(navScope => ({
    dataset: { navScope, section: 'pipelines' }, classList: { toggle(_, active) { this.active = active; } },
    setAttribute(key, value) { this[key] = value; }, removeAttribute(key) { delete this[key]; },
  }));
  const group = {}, mobileGroup = {}, header = { replaceChildren() {} }, title = {};
  const mobile = { querySelector: () => mobileGroup };
  Object.assign(context, {
    document: {
      querySelector: selector => selector === '#pipelines-heading h1' ? title : group,
      querySelectorAll: selector => selector.startsWith('.nav-item') ? links : [],
      getElementById: id => id === 'mobile-page-select' ? mobile : header,
    }, t: text => text, repositoryName: url => url.split('/').pop(),
  });
  get("state.section='pipelines'; state.selectedRepository='lores://host/one'; repositoryBranchSelections.set(state.selectedRepository, 'release'); renderRepositoryContext()");
  assert.equal(group.hidden, false);
  assert.equal(mobileGroup.disabled, false);
  assert.equal(links[0].href, '#pipelines');
  assert.equal(links[0]['aria-current'], 'page');
  assert.equal(links[1].href, '#pipelines?repository=lores%3A%2F%2Fhost%2Fone&branch=release');
  assert.equal(links[1].hidden, false);
  assert.equal(links[1]['aria-current'], undefined);
  assert.equal(mobile.value, 'pipelines');
});
