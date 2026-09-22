// Isolated execution graph fixture. No database, credentials, or persistent writes.
// node tests/execution-preview.mjs; open http://127.0.0.1:4181/#graphs
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
const now = new Date().toISOString();
const repository = "lores://fixture/demo";
const snapshot = { sparse_view: "ServerView", stages: [
  { name: "build", jobs: Array.from({ length: 16 }, (_, i) => `compile-${i + 1}`) },
  { name: "test", jobs: ["unit-tests"] }, { name: "package", jobs: ["package"] }, { name: "publish", jobs: ["publish"] },
], dependencies: [{ job: "unit-tests", needs: ["compile-16"] }] };
function makeRun(name, status, queue_reason) {
  const pipeline = { id: name, pipeline_name: name, repository_url: repository, branch: "main", revision: "a".repeat(64), revision_number: 10,
    category: "server", runner_os: "linux", working_directory: "Server", sparse_view_name: "ServerView", trigger_patterns: ["Server/**"], pipeline_needs: [], status,
    worker_id: status === "queued" ? null : "runner1", created_at: now, started_at: now, finished_at: status === "failed" ? now : null,
    error: status === "failed" ? "unit-tests exited with code 1" : null,
  };
  const jobs = status === "queued" ? [] : snapshot.stages.flatMap(stage => stage.jobs.map((job, index) => ({
    id: `${name}-${job}`, name: job, stage: stage.name, position: stage.name === "build" ? index : 16 + ["test", "package", "publish"].indexOf(stage.name),
    status: stage.name === "build" ? "succeeded" : stage.name === "test" ? (status === "failed" ? "failed" : "running") : status === "failed" ? "skipped" : "queued",
    exit_code: stage.name === "build" ? 0 : stage.name === "test" && status === "failed" ? 1 : null,
    started_at: now, finished_at: stage.name === "build" || status === "failed" ? now : null,
  })));
  return { pipeline, jobs, graph: snapshot, queue_reason };
}
const runs = Object.fromEntries([makeRun("server", "failed"), makeRun("client", "running"), makeRun("waiting-runner", "queued", "runner"), makeRun("waiting-dependency", "queued", "dependencies")].map(run => [run.pipeline.id, run]));
const routes = Object.values(runs).map(({ pipeline }) => ({ ...pipeline, runner_os: "windows", revision: "b".repeat(64), revision_number: 11,
  graph: { stages: [{ name: "new-config", jobs: ["new-job"] }] }, latest_pipeline_id: pipeline.id, latest_created_at: now,
}));
const assets = new Set(["app.js", "app.css", "ci-visual.js", "ci-editor.js", "execution-graph.js", "execution-analysis.js", "management.js", "operations.js", "repository-context.js", "theme.js"]);
// Deterministic analysis timings with a queue delay, longer completed jobs,
// an unfinished job, missing prerequisites, and a changed-configuration baseline.
const at = seconds => new Date(Date.parse(now) + seconds * 1000).toISOString();
function analysisFixture(id) {
  const current = structuredClone(runs[id]);
  current.pipeline.created_at = at(-600);
  current.pipeline.started_at = current.pipeline.status === "queued" ? null : at(-570);
  current.pipeline.finished_at = current.pipeline.status === "failed" ? at(-200) : null;
  current.jobs.forEach((job, i) => {
    job.started_at = ["queued", "skipped"].includes(job.status) ? null : at(-560 + i * 20);
    job.finished_at = ["running", "queued", "skipped"].includes(job.status) ? null : at(-545 + i * 20);
  });
  const previous = structuredClone(current);
  previous.pipeline.id = `${id}-previous`; previous.pipeline.revision_number = 9; previous.pipeline.revision = "9".repeat(64);
  previous.pipeline.status = "succeeded"; previous.pipeline.created_at = at(-1200); previous.pipeline.started_at = at(-1180); previous.pipeline.finished_at = at(-700);
  previous.jobs.forEach((job, i) => { job.status = "succeeded"; job.started_at = at(-1170 + i * 20); job.finished_at = at(-1160 + i * 20); });
  return { observed_at: new Date().toISOString(), current, previous: id === "waiting-runner" ? null : previous,
    upstream: [{ name: id === "client" ? "server" : "client", run_id: id === "client" ? "server" : "client", status: id === "client" ? "failed" : "running", created_at: at(-700) }, { name: "missing", run_id: null, status: null, created_at: null }],
    downstream: [{ name: "waiting-dependency", run_id: "waiting-dependency", status: "queued", created_at: at(-100) }],
    comparable: id !== "waiting-dependency", comparison_warnings: id === "waiting-dependency" ? ["runner_os"] : ["revision"],
    upstream_truncated: false, downstream_truncated: false,
  };
}
createServer(async (req, res) => {
  try {
    const url = new URL(req.url, "http://127.0.0.1:4181");
    const file = url.pathname === "/" ? "index.html" : url.pathname.startsWith("/assets/") && assets.has(url.pathname.slice(8)) ? url.pathname.slice(8) : null;
    if (file) {
      res.writeHead(200, { "content-type": file.endsWith("css") ? "text/css" : file.endsWith("js") ? "text/javascript" : "text/html", "cache-control": "no-store" });
      res.end(await readFile(new URL("../web/" + file, import.meta.url))); return;
    }
    let data = [];
    if (url.pathname === "/api/v1/me") data = { id: "fixture", name: "Execution Preview", email: "fixture@example.test", role: "user" };
    else if (url.pathname === "/api/v1/repositories") data = { repositories: [{ id: "demo", name: "demo", url: repository, storage_backend: "dynamodb_s3" }], server_url: "lores://fixture", storage_backends: ["dynamodb_s3"] };
    else if (url.pathname === "/api/v1/pipeline-history") data = { pipelines: Object.values(runs).map(run => run.pipeline), next_before: null };
    else if (url.pathname === "/api/v1/pipeline-graphs") data = routes;
    else if (url.pathname === "/api/v1/runners") data = [{ id: "runner1", name: "Linux test runner", os: "linux", status: "busy", arch: "x86_64", last_seen: now }];
    else if (url.pathname.endsWith("/insights")) {
      const id = url.pathname.split("/")[4];
      data = analysisFixture(id.replace(/-previous$/, ""));
      if (id.endsWith("-previous")) data = { ...data, current: data.previous, previous: null, upstream: [], downstream: [], comparable: false };
    }
    else if (url.pathname.endsWith("/logs")) {
      const id = url.pathname.split("/")[4], job = url.searchParams.get("job_id"), after = Number(url.searchParams.get("after") || 0);
      data = Array.from({ length: !job ? 2501 : job.includes("compile-16") ? 501 : 3 }, (_, i) => ({ id: i + 1, job_id: job, stream: i === 1 ? "stderr" : "stdout", content: `${job || id}: ${i === 1 ? "fixture diagnostic <script>literal text</script>" : `log ${i + 1}`}\n` })).filter(row => row.id > after).slice(0, Number(url.searchParams.get("limit") || 100));
    } else if (url.pathname.startsWith("/api/v1/pipelines/")) {
      const id = url.pathname.split("/")[4];
      data = id.endsWith("-previous") ? analysisFixture(id.replace(/-previous$/, "")).previous : analysisFixture(id).current;
    }
    res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" }); res.end(JSON.stringify(data));
  } catch (error) { res.writeHead(500); res.end(JSON.stringify({ error: error.message })); }
}).listen(4181, "127.0.0.1", () => console.log("Execution fixture: http://127.0.0.1:4181/#graphs"));
