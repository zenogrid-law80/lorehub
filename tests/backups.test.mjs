import { readFile } from 'node:fs/promises';
import vm from 'node:vm';
import test from 'node:test';
import assert from 'node:assert/strict';
const source = await readFile(new URL('../web/backups.js', import.meta.url), 'utf8');
class Element {
  constructor(tag) { this.tag=tag; this.children=[]; this.listeners={}; this.dataset={}; this.value=''; this.disabled=false; }
  append(...nodes) { for(const node of nodes) { node.parent=this; this.children.push(node); } }
  replaceChildren(...nodes) { this.children=[]; this.append(...nodes); }
  remove() { if(this.parent) this.parent.children=this.parent.children.filter(node=>node!==this); }
  setAttribute(key,value) { this[key]=value; }
  addEventListener(key,action) { (this.listeners[key] ||= []).push(action); }
  async emit(key) { for(const action of this.listeners[key] || []) await action({preventDefault(){}}); }
  showModal() { this.open=true; }
  close() { this.open=false; queueMicrotask(()=>void this.emit('close')); }
  focus() {}
}
function setup(api=async()=>({backups:[],restores:[],configured:true,storage_backends:['local_file']})) {
  const body=new Element('body'), page=new Element('section'); const timers=new Map(); let serial=0;
  const state={locale:'ko',section:'backups',repositoryScope:'lores://host/source',user:{role:'admin'}};
  const ctx=vm.createContext({state,document:{body,createElement:tag=>new Element(tag),getElementById:()=>page,querySelector:()=>null,querySelectorAll:()=>[],hidden:false},
    navigator:{onLine:true},api,csrfToken:()=> 'csrf',toast(){},t:key=>key,repositoryName:url=>url.split('/').at(-1),queueMicrotask,
    setTimeout:(fn,delay)=>{const id=++serial;timers.set(id,{fn,delay});return id;},clearTimeout:id=>timers.delete(id)});
  vm.runInContext(source,ctx); const run=code=>vm.runInContext(code,ctx);
  return {state,run,ctx,page,timers};
}
const backup={id:'archive',repository_name:'source',storage_backend:'local_file'};
test('backup mutation and restore preview send JSON with CSRF protection',async()=>{
  const calls=[]; const {run,ctx}=setup(async(path,options)=>{calls.push({path,options});return {ready:true,file_count:5,target_path:"/backup/restored/source"};});
  await run('backupMutation("/api/v1/test", {name:"test"})');
  assert.equal(calls[0].options.headers['X-CSRF-Token'],'csrf'); assert.equal(calls[0].options.headers['Content-Type'],'application/json');
  run("backupState.data={storage_backends:['local_file']}"); ctx.item=backup;run('openBackupRestore(item)');
  assert.equal(run('backupState.draft.start.disabled'),true);
  await run('previewBackupRestore()');
  assert.equal(calls[1].options.headers['X-CSRF-Token'],'csrf');assert.equal(run('backupState.draft.start.disabled'),false);
  run('backupState.draft.name.value="different"');await run('backupState.draft.name.emit("input")');
  assert.equal(run('backupState.draft.checked'),null);assert.equal(run('backupState.draft.start.disabled'),true);
});
test('late previews cannot authorize a changed target or a closed dialog',async()=>{
  let resolve;const {run,ctx}=setup(()=>new Promise(done=>{resolve=done;}));ctx.item=backup;
  run("backupState.data={storage_backends:['local_file']};openBackupRestore(item)");
  const pending=run('previewBackupRestore()');run('backupState.draft.name.value="changed"');await run('backupState.draft.name.emit("input")');
  resolve({ready:true});await pending;assert.equal(run('backupState.draft.checked'),null);
  const closed=run('previewBackupRestore()');run('closeBackupDialog()');resolve({ready:true});await closed;assert.equal(run('backupState.dialog'),null);
});
test('backup catalog stays global and ignores stale responses after repository selection changes',async()=>{
  let resolve;const calls=[];
  const {run,state,page}=setup(path=>{
    calls.push(path);
    return calls.length===1?new Promise(done=>{resolve=done;}):Promise.resolve({backups:[],restores:[],configured:true});
  });
  const pending=run('loadBackups()');state.repositoryScope='lores://host/other';await run('loadBackups()');
  resolve({backups:[{id:'old'}],restores:[],configured:true});await pending;
  assert.equal(run('backupState.data.backups.length'),0);
  assert.deepEqual(calls,['/api/v1/repository-backups','/api/v1/repository-backups']);
  assert.equal(page.children[0].children[0].children[0].textContent,'모든 백업');
});
test('polling backs off on errors, pauses when hidden and stops on leaving the page',async()=>{
  const {run,state,timers,ctx}=setup(async()=>{throw Error('temporarily unavailable');});
  run("backupState.data={backups:[{status:'running'}],restores:[]}");
  await run('loadBackups()');assert.equal([...timers.values()][0].delay,10000);
  ctx.document.hidden=true;[...timers.values()][0].fn();assert.equal(timers.size,1);
  state.section='overview';run('stopBackupRefresh()');assert.equal(timers.size,0);
});
test('server backup requires downtime acknowledgement and sends no local repository path',async()=>{
  const calls=[];const {run}=setup(async(path,options)=>{calls.push({path,options});return {backups:[],restores:[],configured:true,can_snapshot_server:true};});
  run('backupState.data={backups:[],restores:[],configured:true,can_snapshot_server:true};openServerBackup()');
  assert.equal(run('backupState.dialog.children[4].children[1].disabled'),true);
  run('backupState.dialog.children[2].children[0].checked=true');
  await run('backupState.dialog.children[2].children[0].emit("input")');
  assert.equal(run('backupState.dialog.children[4].children[1].disabled'),false);
  await run('backupState.dialog.children[4].children[1].emit("click")');
  assert.equal(calls[0].path,'/api/v1/server-backups/local');
  assert.deepEqual(JSON.parse(calls[0].options.body),{acknowledge_downtime:true});
});
