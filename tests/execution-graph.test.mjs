import { readFile } from "node:fs/promises";
import vm from "node:vm";
import test from "node:test";
import assert from "node:assert/strict";
const context = vm.createContext({ I18N: { en: {}, ko: {}, "zh-CN": {} } });
vm.runInContext(await readFile(new URL("../web/execution-graph.js", import.meta.url), "utf8"), context);
const get = name => vm.runInContext(name, context);
const plain = value => JSON.parse(JSON.stringify(value));

test("latest route cannot overwrite a historical run or fill in a missing snapshot", () => {
  const route = { revision: "new", runner_os: "windows", error: "old failure", worker_id: "old-runner", graph: { stages: [{ name: "new", jobs: ["new-job"] }] } };
  const detail = { pipeline: { revision: "old", runner_os: "linux", status: "failed" }, graph: null, jobs: [{ id: "actual", stage: "old", name: "old-job", status: "failed" }] };
  const result = get("executionRouteDetail")(route, detail);
  assert.equal(result.graph, null);
  assert.equal(result.pipeline.runner_os, "linux");
  assert.equal(result.pipeline.revision, "old");
  assert.equal(result.route, route);
  assert.equal(get("executionJobs")(result)[0].name, "old-job");
  const configured = get("executionConfigured")(route);
  assert.equal(configured.pipeline.status, "configured");
  assert.equal(configured.pipeline.error, null);
  assert.equal(configured.pipeline.worker_id, null);
  assert.equal(get("executionJobs")(configured)[0].name, "new-job");
  assert.equal(detail.pipeline.status, "failed");
});

test("planned jobs are not invented runtime jobs and actual order wins", () => {
  const graph = { stages: [{ name: "build", jobs: ["old-job"] }] };
  const detail = { pipeline: { status: "running" }, graph, jobs: [] };
  assert.equal(get("executionJobs")(detail)[0].status, "unregistered");
  assert.equal(get("executionJobs")(detail)[0].planned, true);
  detail.jobs = [{ id: "b", position: 2, name: "actual-b", stage: "test" }, { id: "a", position: 1, name: "actual-a", stage: "build" }];
  assert.deepEqual(plain(get("executionJobs")(detail)).map(job => job.id), ["a", "b"]);
  assert.equal(detail.jobs[0].id, "b");
  assert.equal(get("executionSnapshotMismatch")(detail), true);
  detail.jobs = [{ name: "old-job", stage: "build" }];
  assert.equal(get("executionSnapshotMismatch")(detail), false);
});

test("successful and skipped jobs aggregate as complete, failures remain visible", () => {
  const status = names => get("executionStageStatus")(names.map(status => ({ status })));
  assert.equal(status(["succeeded", "skipped"]), "succeeded");
  assert.equal(status(["skipped"]), "skipped");
  assert.equal(status(["running", "queued"]), "running");
  assert.equal(status(["failed", "skipped"]), "failed");
  assert.equal(status(["unregistered"]), "unregistered");
});

test("wait reasons require server evidence and do not invent missing dependency blockers", () => {
  const reason = get("executionWaitReason");
  const detail = { pipeline: { status: "queued", pipeline_needs: ["absent"] } };
  assert.equal(reason(detail, null, []), "Waiting reason unavailable");
  detail.queue_reason = "runner";
  assert.equal(reason(detail, null, []), "Waiting for Runner assignment");
  detail.queue_reason = "dependencies";
  assert.equal(reason(detail, null, []), "Waiting for prerequisite pipelines");
  detail.pipeline.status = "running";
  const jobs = [{ id: "1", status: "running" }, { id: "2", status: "queued" }];
  assert.equal(reason(detail, jobs[1], jobs), "Waiting for a previous job");
  jobs[0].status = "succeeded";
  assert.equal(reason(detail, jobs[1], jobs), "Waiting reason unavailable");
  detail.pipeline.cancel_requested = true;
  assert.equal(reason(detail, jobs[1], jobs), "Cancel requested");
});

test("view state is isolated by run and bounded", () => {
  const memo = get("executionMemo");
  memo("old").zoom = 1.5;
  assert.equal(memo("new").zoom, 1);
  assert.equal(memo("old").zoom, 1.5);
  for (let i = 0; i < 100; i++) memo(String(i));
  assert.equal(get("executionViews.size"), 80);
});
