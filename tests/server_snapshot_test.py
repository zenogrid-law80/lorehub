import copy
import io
import json
import shutil
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock
import importlib.util

HELPER=Path(__file__).resolve().parents[1]/'scripts/repository-folder-archive.py'
spec=importlib.util.spec_from_file_location('server_archive',HELPER)
archive=importlib.util.module_from_spec(spec)
spec.loader.exec_module(archive)

class ServerSnapshots(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name).resolve()
        self.data=self.root/'server'
        (self.data/'immutable/immutable/index').mkdir(parents=True)
        (self.data/'mutable/mutable/index').mkdir(parents=True)
        (self.data/'immutable/immutable/index/objects').write_bytes(b'original server objects')
        (self.data/'mutable/mutable/version').write_bytes(b'\x02\x00\x00\x00')
        (self.data/'mutable/mutable/index/refs').write_bytes(b'original repository refs')
        self.backups=self.root/'backups'; (self.backups/'job').mkdir(parents=True)
        self.output=self.backups/'job/repository.tar.gz'
        self.info={'Id':'a'*64,'Name':'/lorehub-lore-server-local-1','Image':'sha256:test',
            'Config':{'Entrypoint':['loreserver'],'Cmd':None,'Labels':{'com.docker.compose.service':'lore-server-local'},
                'Env':['LORE_CONFIG_PATH=/etc/lore/config','LORE_ENV=docker']},
            'Mounts':[{'Type':'volume','Name':'lorehub_lore_data','Destination':'/var/lib/lore'}],
            'State':{'Running':True,'ExitCode':0,'StartedAt':'original'}}
        self.config='''[immutable_store]
mode="local"
[immutable_store.local]
path="/var/lib/lore/immutable"
[mutable_store]
mode="local"
[mutable_store.local]
path="/var/lib/lore/mutable"
'''
        self.calls=[]; self.copy_failure=False; self.restart_failure=False; self.restart_during_copy=False; self.kill_exit=False
        self.request={'container':'lorehub-lore-server-local-1','root':str(self.backups),'archive':str(self.output)}
        patch=mock.patch.object(archive,'docker',side_effect=self.docker); patch.start(); self.addCleanup(patch.stop)
    def docker(self,*args,**kwargs):
        self.calls.append(args)
        if args[0]=='inspect': return json.dumps([copy.deepcopy(self.info)]).encode()
        if args[0]=='ps': return (self.info['Id'][:12] if self.info['State']['Running'] else '').encode()
        if args[:1]==('cp',) and args[-1]=='-':
            stream=io.BytesIO()
            with tarfile.open(fileobj=stream,mode='w') as output:
                data=self.config.encode(); entry=tarfile.TarInfo('local.toml');entry.size=len(data);output.addfile(entry,io.BytesIO(data))
            return stream.getvalue()
        if args[0]=='stop':
            self.info['State']['Running']=False
            if self.kill_exit: self.info['State']['ExitCode']=137
            return b''
        if args[0]=='start':
            if self.restart_failure: raise ValueError('restart failed')
            self.info['State']['Running']=True;self.info['State']['StartedAt']='restarted';return b''
        if args[0]=='cp':
            if self.copy_failure: raise ValueError('copy failed')
            shutil.copytree(self.data,args[-1],dirs_exist_ok=True)
            if self.restart_during_copy:
                self.info['State']['Running']=True;self.info['State']['StartedAt']='unexpected'
            return b''
        raise AssertionError(args)
    def test_original_server_stores_round_trip_without_working_folder(self):
        result=archive.snapshot_server(self.request)
        self.assertTrue(self.info['State']['Running'])
        self.assertFalse(archive.restart_record(self.backups).exists())
        self.assertEqual(result['kind'],'server-store-tar-gzip')
        self.assertEqual(result['source_path'],'lorehub-lore-server-local-1:/var/lib/lore')
        self.assertEqual(result['server']['volume'],'lorehub_lore_data')
        restored=self.backups/'restored'
        archive.restore_archive({'archive':str(self.output),'archive_sha256':result['archive_sha256'],'target':str(restored)})
        for item in self.data.rglob('*'):
            if item.is_file(): self.assertEqual(item.read_bytes(),(restored/item.relative_to(self.data)).read_bytes())
        self.assertFalse((restored/'.lore').exists())
        self.assertLess(next(i for i,a in enumerate(self.calls) if a[0]=='stop'),next(i for i,a in enumerate(self.calls) if a[0]=='start'))
    def test_copy_failure_restarts_server_and_publishes_nothing(self):
        self.copy_failure=True
        with self.assertRaisesRegex(ValueError,'copy failed'): archive.snapshot_server(self.request)
        self.assertTrue(self.info['State']['Running']);self.assertFalse(self.output.exists())
    def test_killed_shutdown_is_not_a_valid_snapshot(self):
        self.kill_exit=True
        with self.assertRaisesRegex(ValueError,'stop cleanly'): archive.snapshot_server(self.request)
        self.assertTrue(self.info['State']['Running']);self.assertFalse(self.output.exists())
    def test_restart_during_copy_invalidates_snapshot(self):
        self.restart_during_copy=True
        with self.assertRaisesRegex(ValueError,'restarted during'): archive.snapshot_server(self.request)
        self.assertFalse(self.output.exists())
    def test_restart_failure_leaves_durable_recovery_record(self):
        self.restart_failure=True
        with self.assertRaisesRegex(ValueError,'restart failed'): archive.snapshot_server(self.request)
        self.assertTrue(archive.restart_record(self.backups).exists())
        self.restart_failure=False
        self.assertTrue(archive.recover_server({'root':str(self.backups)})['recovered'])
        self.assertTrue(self.info['State']['Running']);self.assertFalse(archive.restart_record(self.backups).exists())
    def test_already_stopped_server_is_not_started(self):
        self.info['State']['Running']=False
        archive.snapshot_server(self.request)
        self.assertFalse(self.info['State']['Running'])
        self.assertFalse(any(call[0] in ('stop','start') for call in self.calls))
    def test_cloud_store_and_environment_overrides_are_rejected_before_stop(self):
        self.config=self.config.replace('mode="local"','mode="aws"')
        with self.assertRaisesRegex(ValueError,'expected local'): archive.snapshot_server(self.request)
        self.assertFalse(any(call[0]=='stop' for call in self.calls))
        self.info['Config']['Env'].append('LORE__MUTABLE_STORE__MODE=aws')
        with self.assertRaisesRegex(ValueError,'overrides'): archive.inspect_server(self.request)
    def test_missing_mutable_store_blocks_publication_but_resumes_server(self):
        (self.data/'mutable/mutable/version').unlink()
        with self.assertRaisesRegex(ValueError,'stores are missing'): archive.snapshot_server(self.request)
        self.assertTrue(self.info['State']['Running']);self.assertFalse(self.output.exists())

if __name__=='__main__': unittest.main()
