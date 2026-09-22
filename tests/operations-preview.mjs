// Local fixture with no credentials, database, or Lore commands.
// node tests/operations-preview.mjs; open http://127.0.0.1:4182/#operations
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
const assets = new Set(["app.js", "app.css", "theme.js", "ci-visual.js", "ci-editor.js", "execution-graph.js", "execution-analysis.js", "management.js", "operations.js", "repository-context.js"]);
const now = new Date().toISOString();
const snapshot = {
  observed_at: now, queue: { queued: 3, running: 2, expired_leases: 1, oldest_wait_seconds: 845 },
  waiting: ["dependencies", "no_runner", "ready"].map((reason, i) => ({ id: `fixture-${i}`, repository_url: "lores://fixture/developer", pipeline_name: ["build-linux", "build-windows", "verify"][i], runner_os: i === 1 ? "windows" : "linux", wait_seconds: 845 - i * 160, reason })),
  runners: [{ os: "linux", online: 2, idle: 1, draining: 0, offline: 1, stopped: 0 }, { os: "windows", online: 0, idle: 0, draining: 1, offline: 1, stopped: 1 }],
  repositories: ["ok", "error", "stale", "unknown"].map((state, i) => ({ resource_id: `repo-${i}`, name: ["developer", "game", "tools", "new-project"][i], state, checked_at: state === "unknown" ? null : now, last_success_at: i < 2 ? now : null, error_code: state === "error" ? "watch_failed" : null, consecutive_failures: state === "error" ? 3 : 0, link_errors: i === 1 ? 2 : 0 })),
  repository_count: 4, storage: { database_bytes: 2147483648, execution_bytes: 10485760, log_bytes: 1073741824 },
};
createServer(async (req, res) => {
  const url = new URL(req.url, "http://127.0.0.1:4182");
  try {
    const file = url.pathname === "/" ? "index.html" : url.pathname.startsWith("/assets/") && assets.has(url.pathname.slice(8)) ? url.pathname.slice(8) : null;
    if (file) {
      res.writeHead(200, { "content-type": file.endsWith(".js") ? "text/javascript" : file.endsWith(".css") ? "text/css" : "text/html", "cache-control": "no-store" });
      res.end(await readFile(new URL(`../web/${file}`, import.meta.url))); return;
    }
    let data = [];
    if (url.pathname === "/api/v1/me") data = { id: "fixture", name: "Operations preview", email: "fixture@example.test", role: "admin" };
    else if (url.pathname === "/api/v1/repositories") data = { repositories: [], server_url: "lores://fixture", storage_backends: ["dynamodb_s3"] };
    else if (url.pathname === "/api/v1/operations") data = snapshot;
    else if (url.pathname === "/api/v1/pipeline-history") data = { pipelines: [], next_before: null };
    res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" }); res.end(JSON.stringify(data));
  } catch (error) { res.writeHead(500); res.end(JSON.stringify({ error: error.message })); }
}).listen(4182, "127.0.0.1", () => console.log("Operations fixture: http://127.0.0.1:4182/#operations"));
