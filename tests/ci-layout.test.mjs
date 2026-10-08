import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";
const context = vm.createContext({ I18N: { en: {}, ko: {}, "zh-CN": {} }, document: { addEventListener() {} }, setTimeout() { return 1; }, clearTimeout() {} });
vm.runInContext(await readFile(new URL("../web/ci-layout.js", import.meta.url), "utf8"), context);
const Session = vm.runInContext("CiLayoutSession", context);
const plain = value => JSON.parse(JSON.stringify(value));
const tick = () => new Promise(resolve => setImmediate(resolve));

test("load is read-only; moves made during a save remain queued and win over stale responses", async () => {
  const requests = [], responses = [];
  const scope = { branch: "main", graph: '["overview"]' };
  const session = new Session(scope, async (method, body) => {
    requests.push({method, body: plain(body)});
    if (method === "GET") return { positions: { a: {x: 1, y: 2} } };
    return new Promise(resolve => responses.push(resolve));
  });
  await session.load();
  assert.equal(requests.length, 1);
  session.move("a", {x: 10, y: 20});
  const first = session.flush();
  session.move("a", {x: 30, y: 40}); session.move("b", {x: 50, y: 60});
  responses.shift()({positions: {a: {x: 10, y: 20}}}); await first;
  assert.deepEqual(plain(session.positions.a), {x: 30, y: 40});
  assert.equal(requests.length, 3);
  assert.equal(requests[1].body.positions.a.x, 10);
  assert.equal(requests[2].body.positions.a.x, 30);
  responses.shift()({positions: {a: {x: 30, y: 40}, b: {x: 50, y: 60}}}); await tick();
  assert.equal(session.saving, false);
  assert.deepEqual(plain(session.pending), {});
});

test("failed saves retain the latest draft for retry with its original branch and graph", async () => {
  let fail = true, payload;
  const session = new Session({branch: "release", graph: '["detail","build"]'}, async (method, body) => {
    if (method === "GET") return {positions: {}};
    payload = body; if (fail) throw new Error("offline"); return {positions: body.positions};
  });
  await session.load(); session.move("check", {x: 1, y: 2}); await session.flush();
  assert.equal(session.error, "offline");
  session.move("check", {x: 7, y: 9}); fail = false; await session.flush();
  assert.equal(payload.branch, "release"); assert.equal(payload.graph, '["detail","build"]');
  assert.equal(payload.positions.check.x, 7); assert.equal(session.error, "");
});

test("separate graph sessions restore their own positions and resetting one leaves the other unchanged", async () => {
  const database = new Map();
  const send = async (method, input) => {
    const key = JSON.stringify([input.branch, input.graph]);
    if (method === "POST") database.set(key, input.reset ? {} : {...database.get(key), ...input.positions});
    return {positions: {...database.get(key)}};
  };
  const a = new Session({branch:"main", graph:'["overview"]'}, send);
  const b = new Session({branch:"other", graph:'["overview"]'}, send);
  await a.load(); await b.load();
  a.move("same", {x:12,y:34}); b.move("same", {x:90,y:80}); await a.flush(); await b.flush();
  const reopened = new Session(a.scope, send); await reopened.load();
  assert.equal(reopened.positions.same.x, 12);
  assert.equal(await reopened.reset(), true); assert.deepEqual(plain(reopened.positions), {});
  await b.load(); assert.equal(b.positions.same.x, 90);
});

test("a failed load cannot be edited as an empty layout and can be retried", async () => {
  let fail = true;
  const session = new Session({}, async () => { if (fail) throw new Error("offline"); return {positions:{a:{x:4,y:5}}}; });
  await session.load(); assert.equal(session.ready, false); assert.equal(await session.reset(), false);
  session.move("a", {x:99,y:99}); assert.deepEqual(plain(session.pending), {});
  fail = false; await session.load(); assert.equal(session.ready, true); assert.equal(session.positions.a.x, 4);
});

test("reset blocks overlapping moves and preserves the saved layout on failure for retry", async () => {
  let respond;
  const session = new Session({}, async method => method === "GET" ? {positions:{a:{x:4,y:5}}} : new Promise((resolve, reject) => { respond = {resolve, reject}; }));
  await session.load();
  const failed = session.reset();
  session.move("a", {x:99,y:99}); assert.deepEqual(plain(session.pending), {});
  respond.reject(new Error("offline")); assert.equal(await failed, false);
  assert.equal(session.positions.a.x, 4); assert.equal(session.retryReset, true); assert.equal(session.resetting, false);
  const retry = session.reset(); respond.resolve({positions:{}}); assert.equal(await retry, true);
  assert.deepEqual(plain(session.positions), {}); assert.equal(session.retryReset, false);
});
