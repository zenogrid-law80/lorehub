// Local, in-memory UI fixture; no credentials, database, or real Runner calls.
// node tests/runner-preview.mjs; open http://127.0.0.1:4183/#runners
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
const assets = new Set(["app.js", "app.css", "theme.js", "ci-visual.js", "ci-editor.js", "execution-graph.js", "execution-analysis.js", "management.js", "operations.js", "repository-context.js"]);
const runners = [
  { id: "fixture-idle", name: "Linux build", os: "linux", busy: false, draining: false, status: "online" },
  { id: "fixture-busy", name: "macOS build", os: "macos", busy: true, draining: false, status: "online" },
  { id: "fixture-drained", name: "Windows maintenance", os: "windows", busy: false, draining: true, status: "online" },
  { id: "fixture-offline", name: "Offline maintenance", os: "linux", busy: false, draining: true, status: "offline" },
  { id: "fixture-stalled", name: "Polling delayed", os: "windows", busy: false, draining: false, status: "online", diagnostic: "poll_stalled" },
  { id: "fixture-stopped", name: "Stopped runner", os: "linux", busy: false, draining: false, status: "offline", diagnostic: "stopped" },
].map(r => ({ ...r, arch: "x86_64", version: "0.2.20", docker_available: true, current_pipeline_id: null }));
createServer(async (req, res) => {
  const url = new URL(req.url, "http://127.0.0.1:4183");
  try {
    const file = url.pathname === "/" ? "index.html" : url.pathname.startsWith("/assets/") && assets.has(url.pathname.slice(8)) ? url.pathname.slice(8) : null;
    if (file) {
      res.writeHead(200, { "content-type": file.endsWith(".js") ? "text/javascript" : file.endsWith(".css") ? "text/css" : "text/html", "cache-control": "no-store" });
      res.end(await readFile(new URL(`../web/${file}`, import.meta.url))); return;
    }
    const drain = url.pathname.match(/^\/api\/v1\/runners\/([^/]+)\/drain$/);
    if (drain && req.method === "POST") {
      const runner = runners.find(r => r.id === drain[1]);
      if (!runner) { res.writeHead(404); res.end(); return; }
      let body = ""; for await (const chunk of req) body += chunk;
      runner.draining = JSON.parse(body).draining;
      res.writeHead(204); res.end(); return;
    }
    let data = [];
    if (url.pathname === "/api/v1/me") data = { id: "fixture", name: "Runner preview", email: "fixture@example.test", role: "admin" };
    else if (url.pathname === "/api/v1/repositories") data = { repositories: [], server_url: "lores://fixture", storage_backends: ["dynamodb_s3"] };
    else if (url.pathname === "/api/v1/runners") data = runners.map(r => ({ ...r,
      diagnostic: r.diagnostic || (r.status === "offline" ? "heartbeat_lost" : r.busy ? (r.draining ? "draining" : "busy") : r.draining ? "paused" : "ready"),
      started_at: new Date(Date.now() - 3600000).toISOString(),
      last_seen: new Date(Date.now() - (r.status === "offline" ? 3600000 : 0)).toISOString(),
      last_claim_at: new Date(Date.now() - (r.diagnostic === "poll_stalled" ? 120000 : r.busy ? 300000 : 1000)).toISOString(),
      stopped_at: r.diagnostic === "stopped" ? new Date(Date.now() - 120000).toISOString() : null,
      observed_at: new Date().toISOString(),
    }));
    else if (url.pathname === "/api/v1/pipeline-history") data = { pipelines: [], next_before: null };
    res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" }); res.end(JSON.stringify(data));
  } catch (error) { res.writeHead(500); res.end(JSON.stringify({ error: error.message })); }
}).listen(4183, "127.0.0.1", () => console.log("Runner fixture: http://127.0.0.1:4183/#runners"));
