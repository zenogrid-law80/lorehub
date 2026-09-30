import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";

const source = await readFile(new URL("../web/management.js", import.meta.url), "utf8");
class Element {
  constructor(tag) { this.tag = tag; this.children = []; this.listeners = {}; this.dataset = {}; this.value = ""; this.disabled = false; this.classList = { add() {} }; }
  append(...nodes) { for (const node of nodes) { node.parent = this; this.children.push(node); } }
  replaceChildren(...nodes) { this.children = []; this.append(...nodes); }
  remove() { if (this.parent) this.parent.children = this.parent.children.filter(node => node !== this); }
  setAttribute(key, value) { this[key] = value; }
  addEventListener(key, action) { (this.listeners[key] ||= []).push(action); }
  async emit(key) { for (const action of this.listeners[key] || []) await action({ preventDefault() {} }); }
  querySelectorAll(selector) { const tags = selector.split(",").map(value => value.trim()); return this.children.flatMap(child => [...(tags.includes(child.tag) ? [child] : []), ...child.querySelectorAll(selector)]); }
  setCustomValidity(value) { this.validationMessage = value; }
  reportValidity() { return true; }
  showModal() { this.open = true; }
  close() { this.open = false; queueMicrotask(() => void this.emit("close")); }
  focus() {}
}
function setup(api = async () => {}) {
  const body = new Element("body");
  const get = id => [body, ...body.querySelectorAll("form, dialog, input, textarea, select, option, button, fieldset, div, p, label, span")].find(node => node.id === id);
  const state = { locale: "en", user: { id: "owner", role: "admin" }, section: "account-groups", query: "" };
  const ctx = vm.createContext({ state, document: { body, createElement: tag => new Element(tag), getElementById: get }, window: { confirm: () => true }, api, csrfToken: () => "csrf", toast() {}, queueMicrotask });
  vm.runInContext(source + "\nrenderGroups = () => {};", ctx);
  const run = code => vm.runInContext(code, ctx);
  run(`management.accounts = [{id:'owner',name:'Owner',email:'owner@example.test'}, {id:'alice',name:'Alice',email:'alice@example.test'}, {id:'bob',name:'Bob',email:'bob@example.test'}];
    management.groups = [{id:'team',name:'Team',description:'Developers',owner_id:'owner',member_ids:['owner','alice']}];`);
  return { ctx, run, get, body, state, open: () => run("openGroupEditor(management.groups[0])"), check: id => get("management-dialog").querySelectorAll("input").find(node => node.type === "checkbox" && node.value === id), form: () => get("management-dialog").children[0] };
}

test("group search finds member emails and owner names, ignoring case and surrounding spaces", () => {
  const { run } = setup();
  assert.equal(run("accountGroupMatches(management.groups[0], ' ALICE@EXAMPLE.TEST ')"), true);
  assert.equal(run("accountGroupMatches(management.groups[0], 'owner')"), true);
  assert.equal(run("accountGroupMatches(management.groups[0], 'missing')"), false);
});

test("member search keeps hidden selections in the saved payload and owner cannot be removed", async () => {
  let payload;
  const s = setup(async (_path, options) => { payload = JSON.parse(options.body); return { ...payload, id: "team", owner_id: "owner" }; });
  s.open();
  assert.equal(s.check("owner").disabled, true);
  s.get("group-member-search").value = "bob";
  await s.get("group-member-search").emit("input");
  assert.equal(s.check("alice"), undefined);
  s.check("bob").checked = true; await s.check("bob").emit("change");
  await s.form().emit("submit");
  assert.deepEqual(payload.member_ids.sort(), ["alice", "bob", "owner"]);
  assert.equal(s.run("management.dirty"), false);
  assert.equal(s.run("management.groups[0].member_ids.length"), 3);
});

test("save failure retains edits and membership for retry", async () => {
  let attempts = 0;
  const s = setup(async (_path, options) => {
    if (++attempts === 1) throw new Error("Save unavailable");
    return { ...JSON.parse(options.body), id: "team", owner_id: "owner" };
  });
  s.open(); s.get("group-name").value = "Renamed"; await s.get("group-name").emit("input");
  s.check("alice").checked = false; await s.check("alice").emit("change");
  await s.form().emit("submit");
  assert.equal(s.get("management-dialog").open, true);
  assert.equal(s.get("group-name").value, "Renamed");
  assert.equal(s.check("alice").checked, false);
  assert.equal(s.check("owner").disabled, true);
  assert.equal(s.run("management.dirty"), true);
  assert.equal(s.form().querySelectorAll("p").some(node => node.textContent === "Save unavailable" && !node.hidden), true);
  await s.form().emit("submit");
  assert.equal(attempts, 2);
  assert.equal(s.run("management.groups[0].name"), "Renamed");
});

test("dirty dialog requires confirmation once when leaving the page", async () => {
  const s = setup(); s.open();
  s.get("group-name").value = "Changed"; await s.get("group-name").emit("input");
  let confirmations = 0;
  s.ctx.window.confirm = () => { confirmations++; return false; };
  assert.equal(s.run("discardManagement()"), false);
  assert.equal(s.get("management-dialog").open, true);
  s.ctx.window.confirm = () => { confirmations++; return true; };
  assert.equal(s.run("discardManagement()"), true);
  assert.equal(confirmations, 2);
  assert.equal(s.run("management.dirty"), false);
});

test("saving locks close and duplicate submission, and stale responses cannot change group data", async () => {
  let resolve, requests = 0;
  const s = setup(() => { requests++; return new Promise(done => { resolve = done; }); });
  s.open(); s.get("group-name").value = "Changed"; await s.get("group-name").emit("input");
  const saving = s.form().emit("submit");
  assert.equal(s.run("activeGroupEditor.requestClose()"), false);
  await s.form().emit("submit"); assert.equal(requests, 1);
  s.state.user = { id: "someone-else", role: "admin" };
  resolve({ id: "team", name: "Changed", description: "", owner_id: "owner", member_ids: ["owner"] });
  await saving;
  assert.equal(s.run("management.groups[0].name"), "Team");
});

test("a non-owner cannot open the editor", () => {
  const s = setup(); s.state.user.id = "bob"; s.open();
  assert.equal(s.get("management-dialog"), undefined);
});

test("member cap includes the owner and prevents submitting more than 200 accounts", async () => {
  let requests = 0;
  const s = setup(async () => { requests++; });
  s.run(`management.accounts.push(...Array.from({length:199}, (_,i) => ({id:'extra-'+i,name:'Member '+i,email:'member'+i+'@example.test'})));
    management.groups[0].member_ids = ['owner', ...management.accounts.filter(a => a.id.startsWith('extra-')).map(a => a.id)];`);
  s.open();
  s.check("alice").checked = true; await s.check("alice").emit("change");
  const save = s.form().querySelectorAll("button").find(button => button.type === "submit");
  assert.equal(save.disabled, true);
  await s.form().emit("submit"); assert.equal(requests, 0);
  s.check("extra-0").checked = false; await s.check("extra-0").emit("change");
  assert.equal(save.disabled, false);
});

test("ownership transfer offers only administrator members and keeps the former owner in the group", async () => {
  let path, payload;
  const s = setup(async (requestedPath, options) => {
    path = requestedPath; payload = JSON.parse(options.body);
    return { id: "team", name: "Team", description: "Developers", owner_id: "alice", member_ids: ["owner", "alice", "bob"] };
  });
  s.run("management.accounts.find(account => account.id === 'alice').role = 'admin'; management.accounts.find(account => account.id === 'bob').role = 'user';");
  s.run("management.groups[0].member_ids.push('bob')");
  assert.deepEqual([...s.run("eligibleGroupOwners(management.groups[0]).map(account => account.id)")], ["alice"]);
  s.run("openGroupOwnershipTransfer(management.groups[0])");
  assert.equal(s.get("group-new-owner").value, "alice");
  await s.form().emit("submit");
  assert.equal(path, "/api/v1/account-groups/team/owner");
  assert.deepEqual(payload, { owner_id: "alice" });
  assert.equal(s.run("management.groups[0].owner_id"), "alice");
  assert.equal(s.run("management.groups[0].member_ids.includes('owner')"), true);
  assert.equal(s.run("management.dirty"), false);
});

test("ownership transfer blocks closing and duplicate submission while saving", async () => {
  let requests = 0, resolve;
  const s = setup(() => { requests++; return new Promise(done => { resolve = done; }); });
  s.run("management.accounts.find(account => account.id === 'alice').role = 'admin'");
  s.run("openGroupOwnershipTransfer(management.groups[0])");
  const pending = s.form().emit("submit");
  assert.equal(s.run("activeGroupTransfer.requestClose()"), false);
  await s.form().emit("submit"); assert.equal(requests, 1);
  s.state.user = { id: "different", role: "admin" };
  resolve({ id: "team", owner_id: "alice", member_ids: ["owner", "alice"] });
  await pending;
  assert.equal(s.run("management.groups[0].owner_id"), "owner");
});

test("ownership transfer failure keeps the selected owner available for retry", async () => {
  let attempts = 0;
  const s = setup(async () => {
    if (++attempts === 1) throw new Error("Owner transfer unavailable");
    return { id: "team", name: "Team", description: "Developers", owner_id: "alice", member_ids: ["owner", "alice"] };
  });
  s.run("management.accounts.find(account => account.id === 'alice').role = 'admin'");
  s.run("openGroupOwnershipTransfer(management.groups[0])");
  await s.form().emit("submit");
  assert.equal(s.get("management-dialog").open, true);
  assert.equal(s.get("group-new-owner").value, "alice");
  assert.equal(s.get("group-new-owner").disabled, false);
  assert.equal(s.run("management.dirty"), false);
  await s.form().emit("submit");
  assert.equal(attempts, 2);
  assert.equal(s.run("management.groups[0].owner_id"), "alice");
});

test("ownership transfer cannot start without another administrator member", () => {
  const s = setup();
  s.run("openGroupOwnershipTransfer(management.groups[0])");
  const save = s.form().querySelectorAll("button").find(button => button.type === "submit");
  assert.equal(save.disabled, true);
  assert.equal(s.get("group-new-owner").children.length, 0);
});
