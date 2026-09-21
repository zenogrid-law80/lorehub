// Pure state tests: no browser, network, database, or production data.
// node --test tests/ci-editor.test.mjs
import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";
const context = vm.createContext({ I18N: { en: {}, ko: {}, "zh-CN": {} }, document: { addEventListener() {} } });
vm.runInContext(await readFile(new URL("../web/ci-editor.js", import.meta.url), "utf8"), context);
const History = vm.runInContext("CiEditHistory", context);
const diff = vm.runInContext("ciLineDiff", context);

test("history groups typing, restores deleted structures, and truncates redo after a new edit", () => {
  const original = { model: { jobs: [{ name: "build", needs: ["prepare"], script: ["echo ok"] }] }, mode: "visual" };
  const history = new History(original, "original");
  history.record({ ...original, selection: { type: "job", jobIndex: 0 } }, "original");
  original.model.jobs[0].name = "changed";
  history.record({ text: "a" }, "a", "name", 100);
  history.record({ text: "ab" }, "ab", "name", 200);
  assert.equal(history.entries.length, 2);
  const restored = history.move(-1);
  assert.equal(restored.model.jobs[0].name, "build");
  assert.equal(restored.model.jobs[0].needs[0], "prepare");
  assert.equal(restored.selection.type, "job");
  assert.equal(history.move(1).text, "ab");
  history.record({ model: { jobs: [] } }, "delete");
  assert.equal(history.move(-1).text, "ab");
  history.record({ mode: "toml", source: "invalid [" }, "toml");
  assert.equal(history.move(1), null);
  assert.equal(history.move(-1).text, "ab");
  assert.equal(history.move(1).source, "invalid [");
});

test("history count and memory limits keep the current state and a usable undo", () => {
  const history = new History({ n: 0 }, "0", 4, 2000);
  for (let n = 1; n <= 20; n++) history.record({ n }, String(n));
  assert.equal(history.entries.length, 4);
  assert.equal(history.move(-1).n, 19);
  const bounded = new History({ text: "a".repeat(100) }, "a", 100, 500);
  for (const text of ["b", "c", "d"]) bounded.record({ text: text.repeat(100) }, text);
  assert.equal(bounded.entries.length, 2);
  assert.equal(bounded.move(-1).text, "c".repeat(100));
});

test("diff reconstructs both versions, including comments, Unicode, blank lines and final newlines", () => {
  for (const [before, after] of [
    ["", "new\n"], ["a\n", ""], ["# 주석\na = 1\n", "a = 2\n"],
    ["a\nb\na\n", "a\nc\na\n"], ["a\n\n", "a\n"], ["same", "same"],
  ]) {
    const rows = diff(before, after);
    assert.equal(rows.filter(row => row.kind !== "add").map(row => row.text).join("\n"), before);
    assert.equal(rows.filter(row => row.kind !== "remove").map(row => row.text).join("\n"), after);
    const changed = rows.filter(row => row.kind !== "same");
    assert.equal(changed.length === 0, before === after);
  }
});

test("large replacement fallback stays exact without quadratic allocation", () => {
  const before = "common\n" + Array.from({ length: 2000 }, (_, i) => `old ${i}`).join("\n") + "\nend";
  const after = "common\n" + Array.from({ length: 2000 }, (_, i) => `new ${i}`).join("\n") + "\nend";
  const rows = diff(before, after, 10);
  assert.equal(rows[0].kind, "same");
  assert.equal(rows.at(-1).kind, "same");
  assert.equal(rows.filter(row => row.kind !== "add").map(row => row.text).join("\n"), before);
  assert.equal(rows.filter(row => row.kind !== "remove").map(row => row.text).join("\n"), after);
});
