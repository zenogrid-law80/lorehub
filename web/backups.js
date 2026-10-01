"use strict";
const backupCopy = {
  all: ["모든 백업", "All backups", "所有备份"],
  intro: ["로컬 Lore 서버의 원본 데이터 전체를 압축합니다. 모든 저장소가 함께 포함되며 S3·DynamoDB 서버는 제외됩니다.", "Archive all original data on the local Lore server. Includes every repository on that server; excludes S3/DynamoDB servers.", "压缩本地 Lore 服务器全部原始数据，包含所有仓库，不包含 S3/DynamoDB 服务器。"],
  create: ["로컬 서버 전체 백업", "Back up local server", "备份本地服务器"],
  serverArchive: ["서버 원본 · 전체 저장소", "Server data · all repositories", "服务器数据 · 所有仓库"],
  legacyArchive: ["이전 작업 폴더 백업 · 서버 백업 아님", "Legacy working folder · not a server backup", "旧工作文件夹 · 非服务器备份"],
  downtime: ["복사하는 동안 로컬 서버의 모든 저장소가 잠시 중단됩니다. 서버는 복사 후 자동 재시작됩니다.", "All repositories on the local server will briefly be unavailable while data is copied. The server restarts automatically after copying.", "复制期间本地服务器所有仓库将短暂不可用，复制后自动重启。"],
  acknowledge: ["전체 저장소의 일시 중단을 확인했습니다", "I acknowledge the brief interruption for all repositories", "我了解所有仓库将短暂中断"],
  serverRestore: ["서버 데이터는 새 폴더에 검증·복원됩니다. 운영 서버에 자동 적용되지 않으며, 서버 전환은 별도 작업입니다.", "Server data is verified and extracted to a new folder. Switching the live server to this data is a separate operation.", "服务器数据将验证并恢复到新文件夹，切换运行中的服务器需单独操作。"],
  snapshotting: ["서버 데이터 복사·압축 중", "Copying and compressing server data", "复制并压缩服务器数据"],
  verify: ["다시 검증", "Verify again", "重新验证"],
  restore: ["새 폴더로 복원", "Restore to a new folder", "恢复到新文件夹"],
  preview: ["복원 내용 확인", "Preview restore", "预览恢复"],
  start: ["복원 시작", "Start restore", "开始恢复"],
  name: ["복원 폴더 이름", "Restore folder name", "恢复文件夹名称"],
  cancel: ["닫기", "Close", "关闭"],
  empty: ["아직 백업이 없습니다.", "No backups yet.", "暂无备份。"],
  unavailable: ["관리자가 백업 저장 위치를 설정하면 사용할 수 있습니다.", "An administrator must configure backup storage before creating backups.", "管理员配置备份存储后即可使用。"],
  private: ["원래 저장소 ID와 원격 연결을 유지한 로컬 복사본을 만듭니다. 원격에 연결하기 전에 복원 내용을 확인하세요.", "The local copy retains its repository ID and remote connection. Inspect it before connecting to the remote.", "本地副本保留仓库 ID 和远程连接。连接远程之前请检查恢复内容。"],
  ready: ["새 폴더로 복원할 수 있습니다.", "Ready to restore into a new folder.", "可以恢复到新文件夹。"],
  blocked: ["복원 폴더 이름과 백업 파일 상태를 확인하세요.", "Check the restore folder name and archive status.", "请检查恢复文件夹名称和归档状态。"],
  running: ["진행 중", "Running", "进行中"],
  succeeded: ["검증 완료", "Verified", "验证完成"],
  failed: ["실패", "Failed", "失败"],
  exporting: ["폴더 압축 중", "Compressing folder", "正在导出"],
  verifying: ["검증 중", "Verifying", "正在验证"],
  validating: ["복원 확인 중", "Validating restore", "正在检查恢复"],
  creating: ["새 저장소 생성 중", "Creating repository", "正在创建仓库"],
  importing: ["데이터 복원 중", "Restoring data", "正在恢复数据"],
  publishing: ["설정 복원 중", "Restoring settings", "正在恢复设置"],
  retry: ["다시 불러오기", "Reload", "重新加载"],
  busy: ["요청 중…", "Submitting…", "正在提交…"],
  files: ["파일", "files", "文件"],
  source: ["대상 폴더", "Source folder", "源文件夹"],
  storage: ["백업 위치", "Backup directory", "备份目录"],
  sourceMissing: ["이 저장소에 대응하는 로컬 폴더를 찾을 수 없습니다. 관리자에게 폴더 설정을 요청하세요.", "No matching local folder is configured for this repository. Ask an administrator to configure it.", "未配置对应的本地仓库文件夹，请联系管理员。"],
  restoreJobs: ["복원 작업", "Restore operations", "恢复操作"],
  quarantine: ["실패한 복원 폴더에는 미완료 표시가 남습니다. 내용을 확인하기 전에는 사용하지 마세요.", "Failed restore folders retain an incomplete marker. Inspect them before use.", "失败的恢复文件夹保留未完成标记，请先检查再使用。"],
};
function bt(key) { return backupCopy[key]?.[state.locale === "ko" ? 0 : state.locale === "zh-CN" ? 2 : 1] || key; }
const backupState = { request: 0, data: null, error: "", timer: null, busy: false, draft: null, dialog: null, previewRequest: 0, failures: 0 };
function localizeBackupNavigation() {
  for (const label of document.querySelectorAll("[data-backup-label]")) label.textContent = bt(label.dataset.backupLabel || "all");
  const option = document.querySelector('#mobile-page-select option[value="backups"]');
  if (option) option.textContent = bt("all");
}
function stopBackupRefresh() { if (backupState.timer) clearTimeout(backupState.timer); backupState.timer = null; }
function scheduleBackupRefresh() {
  stopBackupRefresh();
  if (state.section !== "backups" || !backupState.data) return;
  const active = [...backupState.data.backups, ...backupState.data.restores].some(item => item.status === "running");
  if (active) backupState.timer = setTimeout(() => {
    if (state.section === "backups" && !document.hidden && navigator.onLine !== false) void loadBackups();
    else scheduleBackupRefresh();
  }, Math.min(60000, 5000 * 2 ** backupState.failures));
}
async function loadBackups() {
  const request = ++backupState.request;
  stopBackupRefresh();
  backupState.error = ""; renderBackups();
  try {
    const data = await api("/api/v1/repository-backups");
    if (request !== backupState.request || state.section !== "backups") return;
    backupState.data = data;
    backupState.failures = 0;
  } catch (error) {
    if (request !== backupState.request || state.section !== "backups") return;
    backupState.error = error.message;
    backupState.failures = Math.min(4, backupState.failures + 1);
  }
  renderBackups(); scheduleBackupRefresh();
}
function backupNode(tag, text, className = "") { const node = document.createElement(tag); if (text != null) node.textContent = text; node.className = className; return node; }
function backupButton(label, action, disabled = false) {
  const button = backupNode("button", label, "button button--ghost"); button.type = "button"; button.disabled = disabled;
  button.addEventListener("click", action); return button;
}
function renderBackups() {
  const page = document.getElementById("backups-page"); if (!page || state.section !== "backups") return;
  localizeBackupNavigation(); page.replaceChildren();
  const header = backupNode("header", null, "page-heading"), copy = backupNode("div");
  const title = backupNode("h1", bt("all")); title.id = "backups-title";
  copy.append(title, backupNode("p", bt("intro"))); header.append(copy);
  if (state.user?.role === "admin") header.append(backupButton(bt("create"), () => openServerBackup(), backupState.busy || !backupState.data?.can_snapshot_server || backupState.data.backups.some(item => item.archive_kind === "server-store-tar-gzip" && item.status === "running")));
  page.append(header);
  if (backupState.error) { page.append(backupNode("p", backupState.error, "management-error"), backupButton(bt("retry"), () => void loadBackups())); return; }
  if (!backupState.data) { page.append(backupNode("p", t("dynamic.loading"))); return; }
  if (!backupState.data.configured) page.append(backupNode("p", bt("unavailable"), "management-note"));
  if (backupState.data.backup_directory) page.append(backupNode("p", `${bt("storage")}: ${backupState.data.backup_directory}`, "management-note"));
  if (backupState.data.server_container) page.append(backupNode("p", `${bt("source")}: ${backupState.data.server_container}:/var/lib/lore`, "management-note"));
  if (!backupState.data.backups.length) page.append(backupNode("p", bt("empty"), "management-note"));
  const list = backupNode("div", null, "backup-list");
  for (const item of backupState.data.backups) {
    const card = backupNode("article", null, "repository-card backup-card");
    const content = backupNode("div", null, "repository-card-content");
    content.append(backupNode("p", bt(item.archive_kind === "server-store-tar-gzip" ? "serverArchive" : "legacyArchive"), "management-note"));
    content.append(backupNode("h2", item.repository_name), backupNode("p", new Date(item.created_at).toLocaleString(state.locale)));
    const status = `${bt(item.status)}${item.status === "running" ? ` · ${bt(item.stage)}` : ""}`;
    content.append(backupNode("p", status, `backup-status backup-status--${item.status}`));
    if (item.has_manifest) content.append(backupNode("p", `${item.file_count} ${bt("files")} · ${formatBackupBytes(item.size_bytes)}`));
    if (item.error) content.append(backupNode("p", item.error, "management-error"));
    const actions = backupNode("div", null, "repository-actions");
    if (item.status !== "running" && item.has_manifest) actions.append(backupButton(bt("verify"), () => void verifyBackup(item.id), backupState.busy || !backupState.data.configured));
    if (item.status === "succeeded" && item.has_manifest) actions.append(backupButton(bt("restore"), () => openBackupRestore(item), backupState.busy || !backupState.data.configured));
    card.append(content, actions); list.append(card);
  }
  page.append(list);
  if (backupState.data.restores.length) {
    page.append(backupNode("h2", bt("restoreJobs")), backupNode("p", bt("quarantine"), "management-note"));
    for (const job of backupState.data.restores) page.append(backupNode("p", `${job.target_path || job.target_name} · ${bt(job.status)}${job.status === "running" ? ` · ${bt(job.stage)}` : ""}${job.error ? ` · ${job.error}` : ""}`));
  }
}
function formatBackupBytes(value) { if (value == null) return "—"; const units = ["B", "KB", "MB", "GB", "TB"]; let size = value, i = 0; while (size >= 1024 && i < units.length - 1) { size /= 1024; i++; } return `${size.toFixed(i ? 1 : 0)} ${units[i]}`; }
async function backupMutation(path, data) {
  if (backupState.busy) return;
  backupState.busy = true; renderBackups();
  try { return await api(path, { method: "POST", headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() }, body: JSON.stringify(data || {}) }); }
  finally { backupState.busy = false; renderBackups(); }
}
function openServerBackup() {
  if (!backupState.data?.can_snapshot_server || backupState.busy) return;
  closeBackupDialog();
  const dialog = backupNode("dialog", null, "backup-restore-dialog"); backupState.dialog = dialog;
  const heading = backupNode("h2", bt("create")); heading.id = "server-backup-title"; dialog.setAttribute("aria-labelledby", heading.id);
  const accept = backupNode("input"); accept.type = "checkbox";
  const label = backupNode("label", null, "backup-field"); label.append(accept, backupNode("span", bt("acknowledge")));
  const result = backupNode("p"); result.setAttribute("aria-live", "polite");
  const cancel = backupButton(bt("cancel"), () => { if (!backupState.busy) closeBackupDialog(); });
  const start = backupButton(bt("create"), async () => {
    if (!accept.checked || backupState.busy) return;
    start.disabled = true; cancel.disabled = true;
    try {
      await backupMutation("/api/v1/server-backups/local", { acknowledge_downtime: true });
      closeBackupDialog(); await loadBackups();
    } catch (error) { result.textContent = error.message; start.disabled = !accept.checked; cancel.disabled = false; }
  }, true);
  accept.addEventListener("input", () => { start.disabled = !accept.checked; });
  dialog.addEventListener("cancel", event => { if (backupState.busy) event.preventDefault(); });
  dialog.addEventListener("close", () => { if (backupState.dialog === dialog) { backupState.dialog = null; dialog.remove(); } });
  const actions = backupNode("div", null, "repository-actions"); actions.append(cancel, start);
  dialog.append(heading, backupNode("p", bt("downtime")), label, result, actions); document.body.append(dialog); dialog.showModal(); accept.focus();
}
async function verifyBackup(id) {
  try { await backupMutation(`/api/v1/repository-backups/${encodeURIComponent(id)}/verify`); await loadBackups(); }
  catch (error) { toast(error.message, "error"); }
}
function closeBackupDialog() {
  backupState.previewRequest++;
  if (backupState.dialog) { backupState.dialog.close(); backupState.dialog.remove(); }
  backupState.dialog = null; backupState.draft = null;
}
function openBackupRestore(item) {
  closeBackupDialog(); const dialog = backupNode("dialog", null, "backup-restore-dialog"); backupState.dialog = dialog;
  const heading = backupNode("h2", bt("restore")); heading.id = "backup-restore-title"; dialog.setAttribute("aria-labelledby", heading.id);
  const name = backupNode("input"); name.type = "text"; name.required = true; name.maxLength = 100; name.value = `${item.repository_name.slice(0, 82)}-restored`;
  const label = (text, input) => { const node = backupNode("label", null, "backup-field"); node.append(backupNode("span", text), input); return node; };
  const result = backupNode("p"); result.setAttribute("aria-live", "polite");
  const start = backupButton(bt("start"), () => void submitBackupRestore(), true);
  const preview = backupButton(bt("preview"), () => void previewBackupRestore());
  const cancel = backupButton(bt("cancel"), () => { if (!backupState.busy) closeBackupDialog(); });
  backupState.draft = { id: item.id, name, result, start, preview, cancel, checked: null };
  for (const input of [name]) input.addEventListener("input", () => { backupState.previewRequest++; if (backupState.draft) backupState.draft.checked = null; start.disabled = true; result.textContent = ""; });
  dialog.addEventListener("cancel", event => { if (backupState.busy) event.preventDefault(); });
  dialog.addEventListener("close", () => { if (backupState.dialog === dialog) { backupState.previewRequest++; backupState.dialog = null; backupState.draft = null; dialog.remove(); } });
  dialog.append(heading, backupNode("p", bt(item.archive_kind === "server-store-tar-gzip" ? "serverRestore" : "private")), label(bt("name"), name));
  const actions = backupNode("div", null, "repository-actions"); actions.append(cancel, preview, start);
  dialog.append(result, actions); document.body.append(dialog); dialog.showModal(); name.focus();
}
function restoreDraftInput(draft) { return { name: draft.name.value.trim() }; }
async function previewBackupRestore() {
  const draft = backupState.draft; if (!draft || backupState.busy) return;
  const request = ++backupState.previewRequest, input = restoreDraftInput(draft);
  draft.start.disabled = true; draft.preview.disabled = true; draft.result.textContent = bt("busy");
  try {
    const result = await api(`/api/v1/repository-backups/${encodeURIComponent(draft.id)}/restore-preview`, { method: "POST", headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() }, body: JSON.stringify(input) });
    if (draft !== backupState.draft || request !== backupState.previewRequest) return;
    draft.checked = result.ready ? input : null; draft.start.disabled = !result.ready;
    draft.result.textContent = result.ready ? `${bt("ready")} ${result.target_path} · ${result.file_count} ${bt("files")} · ${formatBackupBytes(result.size_bytes)}` : bt("blocked");
  } catch (error) { if (draft === backupState.draft && request === backupState.previewRequest) draft.result.textContent = error.message; }
  finally { if (draft === backupState.draft) draft.preview.disabled = false; }
}
async function submitBackupRestore() {
  const draft = backupState.draft; if (!draft?.checked || backupState.busy) return;
  draft.start.disabled = true; draft.preview.disabled = true; draft.cancel.disabled = true;
  try {
    const result = await backupMutation(`/api/v1/repository-backups/${encodeURIComponent(draft.id)}/restore`, draft.checked);
    if (result) { closeBackupDialog(); await loadBackups(); }
  } catch (error) { if (draft === backupState.draft) { draft.result.textContent = error.message; draft.checked = null; } }
  finally { if (draft === backupState.draft) { draft.preview.disabled = false; draft.cancel.disabled = false; } }
}
