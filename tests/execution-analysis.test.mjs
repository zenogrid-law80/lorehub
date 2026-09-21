import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";
const context = vm.createContext({ I18N: { en: {}, ko: {}, "zh-CN": {} } });
vm.runInContext(await readFile(new URL("../web/execution-analysis.js", import.meta.url), "utf8"), context);
const get = name => vm.runInContext(name, context);
const at = seconds => new Date(Date.UTC(2026, 0, 1) + seconds * 1000).toISOString();
const job = (name, start, end, status = "succeeded") => ({ id: name, name, stage: "build", status, started_at: start === null ? null : at(start), finished_at: end === null ? null : at(end) });

test("timeline separates queue wait from overlapping job start delay", () => {
  const timeline = get("executionTimeline")({ pipeline_name: "server", status: "succeeded", created_at: at(0), started_at: at(10), finished_at: at(50) }, [job("a", 15, 30), job("b", 35, 50)], at(100));
  assert.equal(timeline.total, 50000);
  assert.equal(timeline.rows[0].wait.ms, 10000);
  assert.equal(timeline.rows[0].run.ms, 40000);
  assert.equal(timeline.rows[1].wait.ms, 5000);
  assert.equal(timeline.rows[2].wait.ms, 25000);
  assert.equal(timeline.rows[2].run.ms, 15000);
});

test("queued and canceled-before-start runs do not invent execution time", () => {
  const queued = { status: "queued", created_at: at(0), started_at: null, finished_at: null };
  let timeline = get("executionTimeline")(queued, [], at(20));
  assert.equal(timeline.rows[0].wait.ms, 20000);
  assert.equal(timeline.rows[0].run, null);
  timeline = get("executionTimeline")({ ...queued, status: "succeeded", finished_at: at(5) }, [], at(20));
  assert.equal(timeline.rows[0].wait, null);
  assert.equal(timeline.rows[0].run, null);
  timeline = get("executionTimeline")({ ...queued, status: "canceled", finished_at: at(5) }, [], at(20));
  assert.equal(timeline.total, 5000);
  assert.equal(timeline.rows[0].wait.ms, 5000);
  assert.equal(timeline.rows[0].run, null);
});

test("live jobs use the observed time; skipped jobs and invalid timestamps remain unknown", () => {
  const pipeline = { status: "running", created_at: at(0), started_at: at(10) };
  const timeline = get("executionTimeline")(pipeline, [job("live", 20, null, "running"), job("later", null, null, "queued"), job("skip", null, 35, "skipped"), job("bad", 40, 30)], at(50));
  assert.equal(timeline.rows[1].run.ms, 30000);
  assert.equal(timeline.rows[2].wait.ms, 40000);
  assert.equal(timeline.rows[2].run, null);
  assert.equal(timeline.rows[3].wait, null);
  assert.equal(timeline.rows[3].run, null);
  assert.equal(timeline.rows[4].run, null);
  assert.equal(get("executionTimeline")({ ...pipeline, created_at: "invalid" }, [], at(50)), null);
  assert.equal(get("executionTimeline")({ ...pipeline, status: "failed" }, [], at(50)), null);
  assert.equal(get("executionTimeline")({ ...pipeline, finished_at: at(60) }, [], at(50)), null);
});

test("comparison ranks slower matching successes and handles zero/missing baselines", () => {
  const rows = get("executionComparisons")([job("fast", 0, 5), job("slow", 0, 20), job("zero", 0, 1), job("new", 0, 10)], [job("slow", 0, 10), job("fast", 0, 10), job("zero", 0, 0)], true);
  assert.equal(rows[0].job.name, "slow");
  assert.equal(rows[0].delta, 10000);
  assert.equal(rows[0].percent, 100);
  assert.equal(rows.find(row => row.job.name === "fast").percent, -50);
  assert.equal(rows.find(row => row.job.name === "zero").percent, null);
  assert.equal(rows.find(row => row.job.name === "new").delta, null);
});

test("partial, failed, mismatched and duplicate jobs never get a slowdown percentage", () => {
  for (const current of [job("a", 0, null, "running"), job("a", 0, 10, "failed"), job("a", 20, 10)]) {
    assert.equal(get("executionComparisons")([current], [job("a", 0, 5)], true)[0].delta, null);
  }
  assert.equal(get("executionComparisons")([job("a", 0, 10)], [job("a", 0, 5)], false)[0].delta, null);
  assert.equal(get("executionComparisons")([job("a", 0, 10)], [job("a", 0, 5), job("a", 0, 6)], true)[0].delta, null);
  assert.equal(get("executionComparisons")([job("a", 0, 10), job("a", 0, 15)], [job("a", 0, 5)], true)[0].delta, null);
  assert.equal(get("executionComparisons")([job("a", 0, 10)], [{ ...job("a", 0, 5), stage: "test" }], true)[0].delta, null);
});
