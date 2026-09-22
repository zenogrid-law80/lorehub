// Isolated UI fixture: no production credentials, database, or Lore commands.
// Run: node tests/web-links-preview.mjs, then open http://127.0.0.1:4179/#repository-links
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
const revision = "a".repeat(64);
const source = { id: "urc-source", name: "developer", url: "lores://fixture/developer", storage_backend: "dynamodb_s3" };
const root = { id: "urc-root", name: "game", url: "lores://fixture/game", storage_backend: "dynamodb_s3" };
const links = [{ path: "Test", source_repository_id: source.id, source_path: "Test", source_branch_id: "source-main-id", source_branch_name: "main", source_revision: revision, latest_revision: "b".repeat(64), auto_update: true, tracking: true, status: "outdated", last_success_at: new Date().toISOString() }];
const operations = [];
const pipelines = [source, root].map(repository => ({
  id: `${repository.name}-run`, pipeline_name: `${repository.name}-build`, repository_url: repository.url,
  branch: "main", revision, revision_number: 1, runner_os: "linux", status: "succeeded",
  created_at: new Date().toISOString(), started_at: new Date().toISOString(), finished_at: new Date().toISOString(),
  pipeline_needs: [], trigger_patterns: [],
}));
pipelines.push(...Array.from({ length: 120 }, (_, i) => ({ ...pipelines[1], id: `recent-${i}`, status: "queued", started_at: null, finished_at: null })));
pipelines.push({ ...pipelines[1], id: "archive-run", pipeline_name: "Archive_100%", branch: "release", status: "failed", created_at: new Date(Date.now() - 86400000).toISOString() });
function historyPage(params) {
  const before = params.get("before");
  const cursorIndex = before ? pipelines.findIndex(row => row.id === before) : -1;
  const rows = pipelines.filter((row, i) => {
    if (before && i <= cursorIndex) return false;
    for (const key of ["repository_url", "branch", "pipeline_name"]) if (params.get(key) && params.get(key) !== row[key]) return false;
    const status = params.get("status");
    if (status && status !== "all" && !(status === "active" ? ["queued", "running"].includes(row.status) : status === "finished" ? ["succeeded", "failed", "canceled"].includes(row.status) : row.status === status)) return false;
    const q = (params.get("q") || "").toLowerCase();
    return !q || [row.id, row.repository_url, row.branch, row.revision, row.status, row.pipeline_name, row.runner_os].some(value => String(value).toLowerCase().includes(q));
  });
  const limit = Number(params.get("limit") || 100);
  return { pipelines: rows.slice(0, limit), next_before: rows.length > limit ? rows[limit - 1].id : null };
}
const assets = { "/": ["index.html", "text/html"], "/app.js": ["app.js", "text/javascript"], "/ci-visual.js": ["ci-visual.js", "text/javascript"], "/ci-editor.js": ["ci-editor.js", "text/javascript"], "/app.css": ["app.css", "text/css"], "/theme.js": ["theme.js", "text/javascript"], "/management.js": ["management.js", "text/javascript"] };
assets["/lore-logo.svg"] = ["lore-logo.svg", "image/svg+xml"];
assets["/repository-tree.js"] = ["repository-tree.js", "text/javascript"];
assets["/repository-context.js"] = ["repository-context.js", "text/javascript"];
assets["/operations.js"] = ["operations.js", "text/javascript"];
assets["/overview.js"] = ["overview.js", "text/javascript"];
assets["/execution-graph.js"] = ["execution-graph.js", "text/javascript"];
assets["/execution-analysis.js"] = ["execution-analysis.js", "text/javascript"];
createServer(async (req, res) => {
  try {
    const url = new URL(req.url, "http://127.0.0.1:4179");
    const asset = assets[url.pathname.replace(/^\/assets/, "")];
    if (asset) {
      res.writeHead(200, { "content-type": asset[1], "cache-control": "no-store" });
      res.end(await readFile(new URL("../web/" + asset[0], import.meta.url))); return;
    }
    let body = "";
    for await (const chunk of req) body += chunk;
    const input = body ? JSON.parse(body) : {};
    let data = [];
    let status = 200;
    if (url.pathname === "/api/v1/me") data = { id: "fixture-user", name: "Link UI test", email: "fixture@example.test", role: "user" };
    else if (url.pathname === "/api/v1/repositories") data = { repositories: [source, root], server_url: "lores://fixture", storage_backends: ["dynamodb_s3"] };
    else if (url.pathname === "/api/v1/pipeline-history") data = historyPage(url.searchParams);
    else if (url.pathname === "/api/v1/overview") data = { observed_at: new Date().toISOString(), summary: { repositories: 2, running: 0, queued: 120, online_runners: 0, failed: 1, waiting: 0, link_errors: 1 }, failed: [pipelines.at(-1)], waiting: [], links: [{ repository_url: root.url, branch: "main", path: "Test" }] };
    else if (url.pathname === "/api/v1/pipeline-graphs") data = pipelines.filter(row => !url.searchParams.has("repository_url") || row.repository_url === url.searchParams.get("repository_url")).map(row => ({ ...row, graph: { stages: [{ name: "build", jobs: ["compile"] }] }, latest_pipeline_id: null }));
    else if (url.pathname.endsWith("/ci-config")) data = { revision, content: null, configuration: null };
    else if (url.pathname === "/api/v1/repository-links/summary") data = [{ resource_id: root.id, branch: "main", count: links.length }];
    else if (url.pathname.endsWith("/tree")) {
      const path = url.searchParams.get("path") || "";
      const folders = {
        "": [{ name: "src", kind: "directory", is_link: false }, { name: "Shared", kind: "directory", is_link: true }, { name: "empty", kind: "directory", is_link: false }, { name: "unavailable", kind: "directory", is_link: true }, { name: "README.md", kind: "file", is_link: false }],
        src: [{ name: "app.js", kind: "file", is_link: false }],
        Shared: [{ name: "Textures", kind: "directory", is_link: false }, { name: "shared.txt", kind: "file", is_link: false }],
        "Shared/Textures": [{ name: "한국어 & texture.png", kind: "file", is_link: false }], empty: [],
      };
      if (path === "unavailable") { status = 403; data = { error: "Linked repository access denied (fixture)" }; }
      else data = folders[path] || [];
    }
    else if (url.pathname.endsWith("/branches")) data = [{ name: "main", revision }, { name: "release", revision }];
    else if (url.pathname.endsWith("/link-operations")) data = url.pathname.includes("/game/") && url.searchParams.get("branch") === "main" ? operations : [];
    else if (url.pathname.endsWith("/retry")) {
      const id = url.pathname.split("/").at(-2);
      data = operations.find(op => op.id === id);
      if (data) {
        Object.assign(data, { status: "succeeded", stage: "complete", result_revision: revision, error: null, updated_at: new Date().toISOString() });
        const request = JSON.parse(data.request);
        links.push({ ...links[0], path: request.path, source_path: request.source_path, auto_update: request.auto_update, status: "current", latest_revision: revision });
      }
    } else if (url.pathname.endsWith("/links/policy")) {
      links.find(link => link.path === input.path).auto_update = input.auto_update; status = 204;
    } else if (url.pathname.endsWith("/links/update")) {
      const link = links.find(link => link.path === input.path);
      Object.assign(link, { status: "current", source_revision: link.latest_revision }); data = { revision };
    } else if (url.pathname.endsWith("/links")) {
      if (req.method === "POST") {
        data = { id: input.operation_id, request: JSON.stringify(input), status: "partial", stage: "root", source_ready: true, source_path_created: true, error: "Failed to add link: Link divergence (isolated test fixture)", created_at: new Date().toISOString(), updated_at: new Date().toISOString() };
        operations.unshift(data); status = 409;
      } else data = { branch: url.searchParams.get("branch"), revision, links: url.pathname.includes("/game/") && url.searchParams.get("branch") === "main" ? links : [] };
    }
    res.writeHead(status, { "content-type": "application/json", "cache-control": "no-store" });
    res.end(status === 204 ? undefined : JSON.stringify(data));
  } catch (error) { res.writeHead(500); res.end(JSON.stringify({ error: error.message })); }
}).listen(4179, "127.0.0.1", () => console.log("Isolated link UI fixture: http://127.0.0.1:4179/#repository-links"));
